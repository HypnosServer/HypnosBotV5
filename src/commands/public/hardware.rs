use std::fs::read_to_string;

use serde::Deserialize;

use crate::commands::prelude::*;

#[derive(Debug, Clone, Deserialize)]
struct Server {
    name: String,
    cpu: String,
    ram: String,
    drives: String,
    gpu: String,
}

#[derive(Default, Clone, Deserialize)]
struct Servers {
    servers: Vec<Server>,
}

#[command(slash_command, prefix_command)]
pub async fn hardware(ctx: Context<'_>) -> Result<(), Error> {
    let hardware_file = read_to_string("data/hardware.json")
        .map_err(|e| format!("Failed to read hardware file: {}", e))?;
    let servers: Servers = serde_json::from_str(&hardware_file)
        .map_err(|e| format!("Failed to parse hardware file: {}", e))?;
    let mut embed = embed(&ctx).await?.title("Hypnos Server Hardware");
    for server in &servers.servers {
        embed = embed
            .field(&server.name, "", false)
            .field("CPU", &server.cpu, true)
            .field("RAM", &server.ram, true)
            .field("Drives", &server.drives, true)
            .field("GPU", &server.gpu, true);
    }

    let reply = CreateReply::default().embed(embed);
    ctx.send(reply).await?;

    Ok(())
}
