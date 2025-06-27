use poise::serenity_prelude::RoleId;

use crate::{Config, commands::prelude::*};

use super::check_role;

/// Gets the invite link for the Hypnos Discord server
#[command(slash_command, prefix_command, check = "check_role")]
pub async fn grinder(ctx: Context<'_>) -> Result<(), Error> {
    let role_id = {
        let data = ctx.serenity_context().data.read().await;
        data.get::<Config>()
            .expect("Config not found in context data")
            .grinder_role
    };
    let author = ctx
        .author_member()
        .await
        .expect("Failed to get author member");
    let http = ctx.http();
    let role = RoleId::new(role_id);
    let mut embed = embed(&ctx).await?;
    if author.roles.contains(&role) {
        author.remove_role(http, role).await?;
        embed = embed
            .title("You is no longer a grinder >:(")
            .description("Toggled grinder role off");
    } else {
        author.add_role(http, role).await?;
        embed = embed
            .title("You is now a grinder :D")
            .description("Toggled grinder role on");
    }
    let reply = CreateReply::default().embed(embed);

    ctx.send(reply).await?;
    Ok(())
}
