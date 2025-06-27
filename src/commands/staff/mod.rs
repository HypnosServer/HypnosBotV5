use crate::config::Config;

use super::prelude::*;

pub(self) async fn check_staff(ctx: Context<'_>) -> Result<bool, Error> {
    {
        let data = ctx.serenity_context().data.read().await;
        let staff = &data.get::<Config>()
            .expect("Config not found in context data")
            .staff;
        
        let member = ctx
            .author_member()
            .await
            .expect("Failed to get author member");
        if staff.contains(&member.user.id.into()) {
            return Ok(true);
        }
    };

    ctx.send(CreateReply::default().content("Staff only :sunglasses:"))
        .await?;
    Ok(false)
}