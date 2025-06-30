use std::{
    collections::HashSet,
    fs::{File, read_to_string},
    io::{BufReader, Read},
    ops::Deref,
    path::PathBuf,
};

use flate2::bufread::GzDecoder;
use futures::{Stream, StreamExt, future};
use poise::serenity_prelude::{collector, CreateButton, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage};
use serde::Deserialize;
use valence_nbt::{Compound, List, Value, from_binary};

use crate::{commands::prelude::*, scoreboard::{Scoreboard, Scoreboards}};

struct Objective {
    name: String,
    display_name: String,
    scores: (String, i32),
}

#[derive(Debug, Clone, Deserialize)]
struct Player {
    uuid: String,
    name: String,
}

pub(super) async fn get_whitelist<'a>(ctx: Context<'a>) -> HashSet<String> {
    let data = ctx.serenity_context().data.read().await;
    let scoreboards = data
        .get::<Scoreboards>()
        .expect("Scoreboards not found in context data");
    scoreboards.get_whitelist().clone()
}

pub(super) async fn get_scoreboard<'a>(ctx: Context<'a>, name: &str) -> Option<Scoreboard> {
    let should_update = {
        let data = ctx.serenity_context().data.read().await;
        let scoreboards = data
            .get::<Scoreboards>()
            .expect("Scoreboards not found in context data");
        if let Some(scoreboard) = scoreboards.scoreboards.get(name) {
            scoreboard.should_update()
        } else {
            true
        }
    };
    if should_update {
        let mut data = ctx.serenity_context().data.write().await;
        let scoreboards = data
            .get_mut::<Scoreboards>()
            .expect("Scoreboards not found in context data");
        scoreboards.load_scoreboard(&name).ok()?;
        scoreboards.load_names().ok()?;
    }
    let data = ctx.serenity_context().data.read().await;
    let scoreboards = data
        .get::<Scoreboards>()
        .expect("Scoreboards not found in context data");
    scoreboards.scoreboards.get(name).cloned()
}

const ACCURACY_THRESHOLD: f64 = 0.5;

// Modified version of the great fuzzy search algorithm
// invented by the great Not Creaturas
fn not_creaturas_furry_search(name: &str, term: &str) -> f64 {
    if name.starts_with(term) {
        return 1.0;
    }
    let mut matches = 0;
    for c in term.chars() {
        if name.contains(c) {
            matches += 1;
        } else {
            matches -= 1;
        }
    }
    matches as f64 / name.len() as f64
}

pub(super) async fn score_autocomplete_board<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    let names = {
        let data = ctx.serenity_context().data.read().await;
        let scoreboards = data
            .get::<Scoreboards>()
            .expect("Scoreboards not found in context data");
        scoreboards.scoreboard_names.names.clone()
    };

    futures::stream::iter(names)
        .filter(move |name| {
            let name = name.to_lowercase();
            let partial = partial.to_lowercase();
            future::ready(not_creaturas_furry_search(&name, &partial) >= ACCURACY_THRESHOLD)
        })
        .map(|name| name.to_string())
}

pub(super) fn format_with_spaces(n: i64) -> String {
    let s = n.abs().to_string();
    let mut result = String::new();
    let len = s.len();

    for (i, c) in s.chars().enumerate() {
        result.push(c);
        let pos_from_end = len - i - 1;
        if pos_from_end % 3 == 0 && pos_from_end != 0 {
            result.push(' ');
        }
    }

    if n < 0 {
        format!("-{}", result)
    } else {
        result
    }
}

pub async fn handle_error(ctx: Context<'_>, error: &str) -> Result<(), Error> {
    ctx.send(CreateReply::default().content(format!("Error: {}", error)))
        .await?;
    Ok(())
}

/// Displays the scoreboard for a given board
///
/// # Arguments
/// * `board` - The name of the board to display scores for
#[command(slash_command, prefix_command)]
pub async fn score(
    ctx: Context<'_>,
    #[description = "The board to display scores for"]
    #[autocomplete = "score_autocomplete_board"]
    board: String,
    whitelist: Option<bool>,
) -> Result<(), Error> {
    let scoreboard = get_scoreboard(ctx.clone(), &board).await;
    let scoreboard = match scoreboard {
        Some(scoreboard) => scoreboard,
        None => {
            ctx.send(
                CreateReply::default().content(format!("No scoreboard found for `{}`", board)),
            )
            .await?;
            return Ok(());
        }
    };

    let nowhitelist;
    let whitelist = if let Some(false) = whitelist {
        nowhitelist = true;
        HashSet::new()
    } else {
        nowhitelist = false;
        get_whitelist(ctx).await
    };

    let base_embed = embed(&ctx).await?
        .title(format!("Scoreboard: {}", board));
    let base_player_string = String::from("```0 Total\n");
    // Format total as 1 230 000 (1.2M)
    let base_score_string = format!(
        "```{} ({:.1}M)\n",
        format_with_spaces(scoreboard.total),
        scoreboard.total as f64 / 1_000_000.0
    );
    let mut player_string = base_player_string.clone();
    let mut score_string = base_score_string.clone();
    let mut embeds = vec![];
    let mut count = 0;
    for (player, score) in scoreboard.scores.iter() {
        if !whitelist.contains(player) && !nowhitelist {
            continue;
        }
        count += 1;


        player_string.push_str(&format!("{} {}\n", count, player));
        score_string.push_str(&format!("{}\n", format_with_spaces(*score as i64)));
        if count % 10 == 0 {
            player_string.push_str("```");
            score_string.push_str("```");
            embeds.push(
                base_embed.clone()
                    .field("Player", player_string, true)
                    .field("Score", score_string, true),
            );
            player_string = base_player_string.clone();
            score_string = base_score_string.clone();
        }

    }

    paginate(ctx, &embeds)
        .await?;

    Ok(())
}

pub async fn paginate<U, E>(
    ctx: poise::Context<'_, U, E>,
    pages: &[CreateEmbed],
) -> Result<(), poise::serenity_prelude::Error> {
    // Define some unique identifiers for the navigation buttons
    let ctx_id = ctx.id();
    let prev_button_id = format!("{}prev", ctx_id);
    let next_button_id = format!("{}next", ctx_id);

    // Send the embed with the first page as content
    let reply = {
        let components = poise::serenity_prelude::CreateActionRow::Buttons(vec![
            CreateButton::new(&prev_button_id).emoji('◀'),
            CreateButton::new(&next_button_id).emoji('▶'),
        ]);

        CreateReply::default()
            .embed(pages[0].clone())
            .components(vec![components])
    };

    ctx.send(reply).await?;

    // Loop through incoming interactions with the navigation buttons
    let mut current_page = 0;
    while let Some(press) = collector::ComponentInteractionCollector::new(ctx)
        // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
        // button was pressed
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        // Timeout when no navigation button has been pressed for 24 hours
        .timeout(std::time::Duration::from_secs(3600 * 24))
        .await
    {
        // Depending on which button was pressed, go to next or previous page
        if press.data.custom_id == next_button_id {
            current_page += 1;
            if current_page >= pages.len() {
                current_page = 0;
            }
        } else if press.data.custom_id == prev_button_id {
            current_page = current_page.checked_sub(1).unwrap_or(pages.len() - 1);
        } else {
            // This is an unrelated button interaction
            continue;
        }

        // Update the message with the new page contents
        press
            .create_response(
                ctx.serenity_context(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .embed(pages[current_page].clone())
                ),
            )
            .await?;
    }

    Ok(())
}
