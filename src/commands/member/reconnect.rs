use poise::serenity_prelude::RoleId;

use crate::{commands::prelude::*, taurus::TaurusChannel, Config};

use super::check_member;

/// Gets the invite link for the Hypnos Discord server
#[command(slash_command, prefix_command, check = "check_member")]
pub async fn reconnect(ctx: Context<'_>) -> Result<(), Error> {
    {
        let data = ctx.serenity_context().data.read().await;
        let (sender, _cache) = data
            .get::<TaurusChannel>()
            .expect("TaurusChannel not found in context data");
        sender.send("__RECONNECT__".to_owned()).await?;
    }
    let embed = embed(&ctx).await?
        .title("Attempting to reconnect")
        .description("Attempting to reconnect to taurus.");

    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;
    Ok(())
}
