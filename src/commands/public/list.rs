use poise::serenity_prelude::prelude::TypeMap;
use tokio::sync::RwLockReadGuard;

use crate::taurus::fetch_latest_with_type;
use crate::{TaurusChannel};

use crate::commands::prelude::*;

pub async fn send_list(data: RwLockReadGuard<'_, TypeMap>) -> Result<String, Error> {
    let (sender, cache) = data
        .get::<TaurusChannel>()
        .expect("TaurusChannel not found in context data");
    sender.send("LIST".to_owned()).await?;
    let res = fetch_latest_with_type(cache.clone(), "LIST").await?[5..].replace(':', ": ");
    return Ok(res);
}

//// Lists the online players on the Hypnos server
#[command(slash_command, prefix_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let data = ctx.serenity_context().data.read().await;
    let res = send_list(data).await?;
    let desc = if res.len() > 1 {
        format!("```{}```", res)
    } else {
        "```No players are currently online.```".to_string()
    };
    let embed = embed(&ctx).await?.title("Online players").description(desc);

    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;
    Ok(())
}
