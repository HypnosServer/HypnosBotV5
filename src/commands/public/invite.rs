use poise::CreateReply;

use crate::commands::prelude::*;

/// Gets the invite link for the Hypnos Discord server
#[command(slash_command, prefix_command)]
pub async fn invite(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let embed = embed(&ctx).await?
        .title("Discord invite link")
        .description("[Link](https://discord.gg/BKadJsM) \nCode: BKadJsM \nFull url: https://discord.gg/BKadJsM");
    let reply = CreateReply::default()
        .embed(embed);
    ctx.send(reply).await?;
    Ok(())
}
    