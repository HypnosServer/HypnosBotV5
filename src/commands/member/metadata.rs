use crate::commands::prelude::*;

#[command(slash_command, prefix_command)]
pub async fn metadata(ctx: Context<'_>, x: i32, y: i32, z: i32) -> Result<(), Error> {
    let embed = embed(&ctx).await?.title("Discord invite link").description(
        "[Link](https://discord.gg/BKadJsM) \nCode: BKadJsM \nFull url: https://discord.gg/BKadJsM",
    );
    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;
    Ok(())
}
