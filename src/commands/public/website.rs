use crate::commands::prelude::*;

/// Displays the website links for Hypnos
#[command(slash_command, prefix_command)]
pub async fn website(ctx: Context<'_>) -> Result<(), Error> {
    let embed = embed(&ctx).await?
        .title("Discord invite link")
        .description("[Main site](http://hypnos.ws)\n[About page](http://hypnos.ws/pages/about)\n[Map](https://hypnos.ws/mapraw)");
    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;
    Ok(())
}
