use std::{
    collections::HashSet,
    fs::{File, read_to_string},
    io::{BufReader, Read},
    ops::Deref,
    path::PathBuf,
};

use flate2::bufread::GzDecoder;
use futures::{Stream, StreamExt, future};
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

fn get_whitelist(path: PathBuf) -> Result<HashSet<String>, String> {
    let file = read_to_string(path).map_err(|e| format!("Failed to read whitelist file: {}", e))?;
    let players: Vec<Player> = serde_json::from_str(&file)
        .map_err(|e| format!("Failed to parse whitelist file: {}", e))?;
    let mut whitelist = HashSet::new();
    for player in players {
        whitelist.insert(player.name);
    }
    Ok(whitelist)
}

async fn get_scoreboard<'a>(ctx: Context<'a>, name: String) -> Option<Scoreboard> {
    let should_update = {
        let data = ctx.serenity_context().data.read().await;
        let scoreboards = data
            .get::<Scoreboards>()
            .expect("Scoreboards not found in context data");
        if let Some(scoreboard) = scoreboards.scoreboards.get(&name) {
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
    scoreboards.scoreboards.get(&name).cloned()
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

async fn score_autocomplete_board<'a>(
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

fn format_with_spaces(n: i64) -> String {
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
) -> Result<(), Error> {
    let scoreboard = get_scoreboard(ctx.clone(), board.clone()).await;
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
    let mut player_string = String::from("```0 Total\n");
    // Format total as 1 230 000 (1.2M)
    let mut score_string = format!(
        "```{} ({:.1}M)\n",
        format_with_spaces(scoreboard.total),
        scoreboard.total as f64 / 1_000_000.0
    );
    for (i, (player, score)) in scoreboard.scores.iter().enumerate() {
        player_string.push_str(&format!("{} {}\n", i + 1, player));
        score_string.push_str(&format!("{}\n", score));
    }
    player_string.push_str("```");
    score_string.push_str("```");
    let embed = embed(&ctx)
        .await?
        .title(format!("Score: {}", board))
        .field("Player", player_string, true)
        .field("Score", score_string, true);
    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;

    Ok(())
}
