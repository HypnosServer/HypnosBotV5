use std::{
    io::{BufReader, Read},
    path::PathBuf,
};

use flate2::{GzBuilder, bufread::GzDecoder};
use valence_nbt::{Value, from_binary};

use crate::{Config, commands::prelude::*};

#[command(slash_command, prefix_command)]
pub async fn worldsize(ctx: Context<'_>) -> Result<(), Error> {
    let worlds = {
        let data = ctx.serenity_context().data.read().await;
        let config = data
            .get::<Config>()
            .expect("Config not found in context data");
        config.worlds.clone()
    };

    let mut embed = embed(&ctx).await?.title("World File Size");
    for world in worlds {
        let child_process = std::process::Command::new("du")
            .arg("-sh")
            .arg(&world.path)
            .output()
            .map_err(|_e| format!("Probably not good"))?;
        if !child_process.status.success() {
            return Err("Failed to get world size".into());
        }
        let size = String::from_utf8_lossy(&child_process.stdout)
            .split('\t')
            .next()
            .unwrap_or("Unknown")
            .to_string();
        embed = embed.field(world.name.clone(), size.to_string(), false);
    }
    let reply = CreateReply::default().embed(embed);

    ctx.send(reply).await?;
    Ok(())
}
