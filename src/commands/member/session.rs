/// NC suboptimal protocol implementation for this one ngl

#[derive(Deserialize, Clone)]
struct Rcon;

#[derive(Deserialize, Clone)]
struct Game {
    chat_bridge: Option<bool>,
    backup_interval: Option<u64>,
    backup_keep: Option<u64>,
}

#[derive(Deserialize, Clone)]
struct Session {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub game: Option<Game>,
    pub rcon: Option<Rcon>,
}

use serde::Deserialize;

use crate::{TaurusChannel, fetch_latest_with_type};

use crate::commands::prelude::*;

use super::check_role;

//// Lists the online players on the Hypnos server
#[command(slash_command, prefix_command, check = "check_role")]
pub async fn session(ctx: Context<'_>) -> Result<(), Error> {
    let cache = {
        let data = ctx.serenity_context().data.read().await;
        let (sender, cache) = data
            .get::<TaurusChannel>()
            .expect("TaurusChannel not found in context data");
        sender.send("LIST_SESSIONS".to_owned()).await?;
        cache.clone()
    };
    let res = &fetch_latest_with_type(cache, "LIST_SESSIONS").await?[14..];
    let sessions: Vec<Session> = serde_json::from_str(res).map_err(|e| "Bruh taurus fail")?;
    let mut embed = embed(&ctx).await?.title("Taurus Sessions");
    for s in sessions {
        embed = embed.field(format!("Name: {}", s.name), "", false);
        if let Some(desc) = s.description {
            embed = embed.field("Description", desc, false);
        }
        embed = embed.field("Host", s.host, true);

        if let Some(_) = s.rcon {
            embed = embed.field("RCON", "Enabled", true);
        } else {
            embed = embed.field("RCON", "Disabled", true);
        }
        if let Some(game) = s.game {
            if let Some(chat_bridge) = game.chat_bridge {
                embed = embed.field("Chat Bridge", chat_bridge.to_string(), true);
            }
            if let Some(backup_interval) = game.backup_interval {
                if backup_interval > 0 {
                    embed = embed.field(
                        "Backup Interval",
                        format!("{} seconds", backup_interval),
                        true,
                    );
                }
            }
            if let Some(backup_keep) = game.backup_keep {
                if backup_keep > 0 {
                    embed =
                        embed.field("Backup Keep Time", format!("{} seconds", backup_keep), true);
                }
            }
        }
    }
    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;
    Ok(())
}
