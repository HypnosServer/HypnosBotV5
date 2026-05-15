use poise::serenity_prelude::Context;
use tokio::sync::mpsc::Sender;

use crate::CurrentIngameBoard;
use crate::scoreboard::{Scoreboards, get_scoreboard};
use crate::taurus::TaurusChannel;

use crate::commands::public::send_list;

struct Calc<'a> {
    tx: &'a Sender<String>,
    server: &'a str,
}

impl<'a> Calc<'a> {
    pub async fn new(tx: &'a Sender<String>, server: &'a str) -> Self {
        tx.send(format!(
            "RCON {} scoreboard objectives add calc dummy",
            server,
        ))
        .await
        .expect("Taurus dead");
        return Self { tx, server };
    }

    pub async fn add_player(&self, player: &str, board: &str) {
        self.tx
            .send(format!(
                "RCON {} scoreboard players operation val calc += {} {}",
                self.server, player, board
            ))
            .await
            .unwrap();
    }

    pub async fn add(&self, val: i64) {
        self.tx
            .send(format!(
                "RCON {} scoreboard players add val calc {}",
                self.server, val
            ))
            .await
            .unwrap();
    }

    pub async fn remove_player(&self, player: &str, board: &str) {
        self.tx
            .send(format!(
                "RCON {} scoreboard players operation val calc -= {} {}",
                self.server, player, board
            ))
            .await
            .unwrap();
    }

    pub async fn remove(&self, val: i64) {
        self.tx
            .send(format!(
                "RCON {} scoreboard players remove val calc {}",
                self.server, val
            ))
            .await
            .unwrap();
    }

    pub async fn set(&self, val: i64) {
        self.tx
            .send(format!(
                "RCON {} scoreboard players set val calc {}",
                self.server, val
            ))
            .await
            .unwrap();
    }

    pub async fn set_other_total(&self, board: &str) {
        self.tx
            .send(format!(
                "RCON {} scoreboard players operation Total {} = val calc",
                self.server, board
            ))
            .await
            .unwrap();
    }
}

pub async fn get_data(ctx: &Context) -> Option<(String, Vec<String>, i64, i64)> {
    let (player_lists, current) = {
        let data = ctx.data.read().await;
        let board = data.get::<CurrentIngameBoard>().unwrap();
        if board.is_none() {
            return None;
        }
        let board = board.as_ref().unwrap().clone();
        (send_list(data).await, board)
    };
    let (players, removes, total) = {
        let mut smp_players = Vec::new();
        for line in player_lists {
            if !line.contains("SMP") {
                continue;
            }
            let players = line.split(": ").last().unwrap();
            for player in players.split(", ") {
                if !player.is_empty() {
                    smp_players.push(player.to_string());
                }
            }
        }
        let Some(scoreboard) = get_scoreboard(&ctx, &current).await else {
            return None;
        };
        let mut removes = 0;
        for player in &smp_players {
            let Some(score) = scoreboard.get_score(player) else {
                continue;
            };
            removes += score;
        }
        (smp_players, removes, scoreboard.total)
    };
    Some((current, players, removes, total))
}

pub async fn scoreboard_update(ctx: &Context) {
    let mut data = None;
    let (tx, _) = ctx
        .data
        .read()
        .await
        .get::<TaurusChannel>()
        .unwrap()
        .clone();
    let calc = Calc::new(&tx, "SMP").await;
    let mut i = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        i += 1;
        if i % 60 == 0 {
            data = get_data(ctx).await;
            i = 0;
        }
        let Some((current, players, removes, total)) = &data else {
            continue;
        };

        calc.set(*total).await;
        calc.remove(*removes).await;
        for player in players {
            calc.add_player(&player, &current).await;
        }
        calc.set_other_total(&current).await;
    }
}
