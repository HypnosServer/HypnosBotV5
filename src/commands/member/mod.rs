mod backup;
mod grinder;
mod session;

pub use backup::backup;
pub use grinder::grinder;
pub use session::session;

use poise::serenity_prelude::RoleId;

use crate::Config;

use super::prelude::*;

pub(self) async fn check_role(ctx: Context<'_>) -> Result<bool, Error> {
    let member_role = {
        let data = ctx.serenity_context().data.read().await;
        data.get::<Config>()
            .expect("Config not found in context data")
            .member_role
    };
    let member = ctx
        .author_member()
        .await
        .expect("Failed to get author member");
    if member.roles.contains(&RoleId::new(member_role)) {
        return Ok(true);
    }
    ctx.send(CreateReply::default().content("Member only :sunglasses:"))
        .await?;
    Ok(false)
}
