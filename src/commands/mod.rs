use std::sync::{Arc, RwLock};

use poise::serenity_prelude::{prelude::TypeMap, CreateEmbed, CreateEmbedFooter};

use crate::Config;

pub mod public;

type Data = ();


pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn embed(
    ctx: &Context<'_>,
) -> Result<CreateEmbed, Error> {
    let serenity_ctx = ctx.serenity_context();
    let data = serenity_ctx.data.read().await;
    let config = data.get::<Config>().ok_or("Config not found")?;
    let opts = &config.embed_opts;
    let footer = CreateEmbedFooter::new(&opts.footer_text)
        .icon_url(&opts.footer_icon_url);
    let hex_colour: u32 = u32::from_str_radix(&opts.colour[1..], 16)
        .map_err(|_| "Failed to parse hex colour")?;
    let embed = CreateEmbed::default()
        .colour(hex_colour)
        .footer(footer);
    
    Ok(embed)
}

mod prelude {
    pub use super::{Context, Error, embed};
    pub use poise::command;
}