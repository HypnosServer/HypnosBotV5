use std::{
    io::{BufReader, Read},
    path::PathBuf,
};

use flate2::{GzBuilder, bufread::GzDecoder};
use valence_nbt::{Value, from_binary};

use crate::{Config, commands::prelude::*};

#[command(slash_command, prefix_command)]
pub async fn tps(ctx: Context<'_>) -> Result<(), Error> {
    let worlds = {
        let data = ctx.serenity_context().data.read().await;
        let config = data
            .get::<Config>()
            .expect("TaurusChannel not found in context data");
        config.worlds.clone()
    };

    let mut embed = embed(&ctx).await?.title("Hypnos Server TPS");
    for world in worlds {
        let mut new = None;
        for file_name in ["level.dat", "leve.dat_old"] {
            let path = PathBuf::from(&world.path).join(file_name);
            if !path.exists() {
                continue;
            }
            let file =
                std::fs::File::open(&path).map_err(|e| format!("unable to access world files"))?;
            let reader = BufReader::new(file);
            let mut buf = Vec::new();
            let mut decoder = GzDecoder::new(reader);
            decoder
                .read_to_end(&mut buf)
                .map_err(|e| format!("failed to read world files"))?;
            let (nbt, _) = from_binary::<String>(&mut buf.as_slice())
                .map_err(|e| format!("failed to parse world files"))?;
            let Some(Value::Compound(data)) = nbt.get("Data") else {
                continue;
            };
            let Some(Value::Long(last_played_value)) = data.get("LastPlayed") else {
                continue;
            };
            if file_name == "level.dat" {
                new = Some(*last_played_value);
            } else {
                let Some(new) = new else {
                    continue;
                };
                let mut tps = (45.0 / ((new - last_played_value) as f64 / 1000.0)).round() * 20.0;
                if tps < 0.0 {
                    tps = 0.0;
                }
                if tps > 20.0 {
                    tps = 20.0;
                }
                embed = embed.field(world.name.clone(), format!("{:.2} TPS", tps), false);
            }
        }
    }
    let reply = CreateReply::default().embed(embed);

    ctx.send(reply).await?;
    Ok(())
}
