use poise::serenity_prelude::Context;

use crate::scoreboard::Scoreboards;
use crate::taurus::TaurusChannel;
use crate::CurrentIngameBoard;

use crate::commands::public::send_list;



pub async fn scoreboard_update(ctx: &Context) {
    let mut count = 0;
    loop {
        // Sleep for 1s
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let (tx, _) = ctx.data.read().await.get::<TaurusChannel>().unwrap().clone();

        let (player_lists, current) = {
            let data = ctx.data.read().await;
            let board = data.get::<CurrentIngameBoard>().unwrap();
            if board.is_none() {
                continue;
            }
            let board = board.as_ref().unwrap().clone();
            (send_list(data).await, board)
        };
        let (adds, removes, total) = {
            let mut data = ctx.data.write().await;
            let scoreboards = data.get_mut::<Scoreboards>().unwrap();
            let mut smp_players = Vec::new();
            for line in player_lists {
                if !line.contains("SMP") {
                    continue;
                }
                let players = line.split(": ").last().unwrap();
                for player in players.split(", ") {
                    smp_players.push(player.to_string());
                }
            }
            smp_players = smp_players.into_iter().filter(|p| !p.is_empty()).collect();
            let Ok(scoreboard) = scoreboards.get_scoreboard(&current) else {
                continue;
            };
            let mut removes = Vec::new();
            for player in &smp_players {
                let Some(score) = scoreboard.get_score(player) else {
                    continue;
                };
                removes.push(score);
            }
            (smp_players, removes, scoreboard.total)
        };
        tx.send(format!(
                "RCON {} scoreboard players set Total {} {}",
                "SMP", current, total
        )).await.unwrap();
        for remove in removes {
            tx.send(format!(
                    "RCON {} scoreboard players remove Total {} {}",
                    "SMP", current, remove
            )).await.unwrap();
        }
        for add in adds {
            tx.send(format!(
                    "RCON {} scoreboard players operation Total {} += {} {}",
                    "SMP", current, add, current
            )).await.unwrap();
        }
    }

}
