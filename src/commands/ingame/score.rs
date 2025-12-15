use crate::{commands::{prelude::Error, public::{get_scoreboard, search_scoreboards, SearchFunction}}, scoreboard::ScoreboardName, taurus::{mc_format, TaurusChannel}};
use futures::{Stream, StreamExt, future};
use poise::serenity_prelude::Context;

fn build_search_results(entries: Vec<ScoreboardName>, max: usize) -> String {
    let mut components = Vec::new();

    components.push(format!(
        r#"{{"text":"Search results:\n", "bold": true, "color":"dark_blue"}}"#,
    ));

    for name in entries.iter().take(max) {
        components.push(format!(
            r#"{{text":"  {display}","color":"blue","clickEvent":{{"action":"suggest_command","value":"/scoreboardPublic objectives setdisplay sidebar {real}"}},"hoverEvent":{{"action":"show_text","value":[{{"text":"{real}"}}]}}}}"#,
            display = name.display,
            real = name.real,
        ));
    }

    format!(r#"[{}]"#, components.join(","))
}


pub async fn score(ctx: &Context, server: &str, board: &str) -> Result<(), Error> {
    let scoreboard = get_scoreboard(ctx, board).await;
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
                search_scoreboards(ctx, board, SearchFunction::contains(true, true))
                    .await
                    .collect::<Vec<ScoreboardName>>()
                    .await;
            //if search_results.is_empty() {
            //    return Ok(());
            //}
            search_results.sort_by(|a, b| a.real.cmp(&b.real));
            let result_string = build_search_results(search_results, 10);
            let cmd = format!("RCON {} tellraw @a {}", server, result_string);
            println!("{}", cmd);
            tx.send(cmd).await.expect("Taurus dead");
            return Ok(());
        }
    };
    tx.send(format!("RCON {} scoreboard objectives setdisplay sidebar {}", server, board)).await.expect("Taurus dead");
    Ok(())
}
