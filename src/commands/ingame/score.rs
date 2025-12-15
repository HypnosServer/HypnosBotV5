use crate::{
    commands::{
        prelude::Error,
        public::{SearchFunction, get_scoreboard, search_scoreboards},
    },
    scoreboard::ScoreboardName,
    taurus::{TaurusChannel, mc_format},
};
use futures::{Stream, StreamExt, future};
use poise::serenity_prelude::Context;

fn build_search_results(entries: Vec<ScoreboardName>, max: usize) -> String {
    let mut components = Vec::new();

    components.push(format!(
        r#"{{"text":"Search results:\n", "bold": true, "color":"dark_blue"}}"#,
    ));

    for (i, name) in entries.iter().take(max).enumerate() {
        let last = (i + 1) == entries.len() || (i + 1) == max;
        let display = if last {
            &name.display
        } else {
            &format!("{}\n", name.display)
        };
        components.push(format!(
            r#"{{"text":"  {display}","color":"blue","clickEvent":{{"action":"suggest_command","value":"/scoreboardPublic objectives setdisplay sidebar {real}"}},"hoverEvent":{{"action":"show_text","value":[{{"text":"{real}"}}]}}}}"#,
            real = name.real,
        ));
    }

    format!(r#"[{}]"#, components.join(","))
}

pub async fn score(ctx: &Context, server: &str, board: &str) -> Result<(), Error> {
    let board = board.replace("\\_", "_");
    let scoreboard = get_scoreboard(ctx, &board).await;
    let tx = {
        let data = ctx.data.read().await;
        let (tx, _rx) = data
            .get::<TaurusChannel>()
            .expect("TaurusChannel not found");
        tx.clone()
    };
    let scoreboard = match scoreboard {
        Some(scoreboard) => scoreboard,
        None => {
            let mut search_results =
                search_scoreboards(ctx, &board, SearchFunction::contains(true, true))
                    .await
                    .collect::<Vec<ScoreboardName>>()
                    .await;
            if search_results.is_empty() {
                let text = r#"{{"text":"No search results", "bold": true, "color":"dark_blue"}}"#;
                let cmd = format!("RCON {} tellraw @a {}", server, text);
                tx.send(cmd).await.expect("Taurus dead");
                return Ok(());
            }
            search_results.sort_by(|a, b| a.real.cmp(&b.real));
            let result_string = build_search_results(search_results, 5);
            let cmd = format!("RCON {} tellraw @a {}", server, result_string);
            tx.send(cmd).await.expect("Taurus dead");
            return Ok(());
        }
    };
    tx.send(format!(
        "RCON {} scoreboard objectives setdisplay sidebar {}",
        server, board
    ))
    .await
    .expect("Taurus dead");
    Ok(())
}
