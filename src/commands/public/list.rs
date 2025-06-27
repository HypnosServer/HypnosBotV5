use crate::taurus::fetch_latest_with_type;
use crate::{TaurusChannel};

use crate::commands::prelude::*;

//// Lists the online players on the Hypnos server
#[command(slash_command, prefix_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let cache = {
        let data = ctx.serenity_context().data.read().await;
        let (sender, cache) = data
            .get::<TaurusChannel>()
            .expect("TaurusChannel not found in context data");
        sender.send("LIST".to_owned()).await?;
        cache.clone()
    };
    let res = &fetch_latest_with_type(cache, "LIST").await?[5..].replace(':', ": ");
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
