use std::{io::{BufRead, Read, Write}, path::PathBuf, process::Command, sync::Arc};

use poise::serenity_prelude::{ChannelId, Context, CreateEmbedFooter, CreateMessage, EditMessage, GuildId, Http, Message, MessageId};
use tokio::time::sleep;
use valence_anvil::RegionFolder;
use valence_nbt::{Compound, Value};

use crate::config::Config;

struct World {
    ow: RegionFolder,
    nether: RegionFolder,
    end: RegionFolder,
}

fn get_block(x: i64, y: i64, z: i64, chunk: Compound) -> Option<(u8, u8)> {
    let Some(Value::Compound(chunk_data)) = chunk.get("Level") else {
        return None;
    };
    let Some(Value::List(subchunks)) = chunk_data.get("Sections") else {
        return None;
    };
    let subchunk = y / 16;
    let subchunk_index = subchunk as usize;
    if subchunk_index >= subchunks.len() {
        return None;
    }
    let subchunk = subchunks.get(subchunk_index)?;
    let Value::Compound(subchunk_data) = subchunk.to_value() else {
        return None;
    };
    let x_bounded = x.rem_euclid(16);
    let y_bounded = y.rem_euclid(16);
    let z_bounded = z.rem_euclid(16);
    let block_index = (y_bounded * 16 + z_bounded) * 16 + x_bounded;
    let Some(Value::ByteArray(blocks)) = subchunk_data.get("Blocks") else {
        return None;
    };

    let Some(Value::ByteArray(data)) = subchunk_data.get("Data") else {
        return None;
    };

    let block_id = blocks.get(block_index as usize)?;
    let rest = block_index % 2;
    // Data is packed in 4-bit nibbles, so we need to extract the correct nibble
    let data_index = block_index / 2;
    let data_value = if rest == 0 {
        data.get(data_index as usize)? & 0x0F
    } else {
        data.get(data_index as usize)? >> 4 & 0x0F
    };
    Some((*block_id as u8, data_value as u8))

}

impl World {
    fn new(world_path: PathBuf) -> Self {
        let ow = RegionFolder::new(world_path.join("region"));
        let nether = RegionFolder::new(world_path.join("DIM-1/region"));
        let end = RegionFolder::new(world_path.join("DIM1/region"));

        World { ow, nether, end }
    }
}

fn run_loop(world: &mut World) -> Vec<String> {
    return Vec::new();
    let mut child_process = Command::new("/usr/bin/env")
        .arg("python3")
        .arg("anvil_script/anvil.py")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start python script");

    let mut stdin = child_process.stdin.take().expect("Failed to open stdin");
    let stdout = child_process.stdout.take().expect("Failed to open stdout");

    let mut buf_reader = std::io::BufReader::new(stdout);

    // while child is running
    let mut prints = Vec::new();
    loop {
        // Read a line from stdin
        if let Ok(Some(status)) = child_process.try_wait() {
            if status.success() {
                println!("Child process exited successfully.");
            } else {
                eprintln!("Child process exited with an error.");
            }
            break; // Exit the loop if the child process has exited
        }
        let mut input = String::new();
        if let Err(_) = buf_reader.read_line(&mut input) {
            break; // Exit the loop on error
        }
        //let mut input_line = String::new();
        //if let Err(_) = std::io::stdin().read_line(&mut input_line) {
        //    break; // Exit the loop on error
        //}
        let input = input.trim();
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            continue; // Skip empty input
        }
        match parts[0] {
            "GET" => {
                if parts.len() != 5 {
                    continue;
                }
                let dim: &str = parts[1];
                let x: i64 = match parts[2].parse() {
                    Ok(val) => val,
                    Err(_) => {
                        continue;
                    }
                };
                let y: i64 = match parts[3].parse() {
                    Ok(val) => val,
                    Err(_) => {
                        continue;
                    }
                };
                let z: i64 = match parts[4].parse() {
                    Ok(val) => val,
                    Err(_) => {
                        continue;
                    }
                };
                let region = match dim {
                    "overworld" => &mut world.ow,
                    "nether" => &mut world.nether,
                    "end" => &mut world.end,
                    _ => {
                        continue;
                    }
                };
                let chunk = region.get_chunk(x.div_euclid(16) as i32, z.div_euclid(16) as i32);
                if let Ok(Some(chunk)) = chunk {
                    if let Some((block_id, data)) = get_block(x, y, z, chunk.data) {
                        // Send the block ID and data to the python script
                        let response = format!("{} {}\n", block_id, data);
                        if let Err(e) = stdin.write_all(response.as_bytes()) {
                        }
                    }
                }
            }
            "PRINT" => {
                if parts.len() == 1 {
                    continue;
                }
                let message = input.split_once(' ').map_or(input, |(_, msg)| msg);
                prints.push(message.to_string());
            }
            _ => {
                println!("Unknown command: {}", parts[0]);
            }
        }
    }
    // Wait for the child process to finish
    if let Err(e) = child_process.wait() {
        eprintln!("Failed to wait for child process: {}", e);
    }
    prints
}

pub async fn run_anvil(
    ctx: &Context,
) {
    let mut message: Option<Message> = None;
    let (channel, world_path) = {
        let data = ctx.data.read().await;
        let config = data.get::<Config>().expect("Config not found");
        let channel = ChannelId::new(config.info_channel);
        let world_path = config.get_world_path("SMP")
            .expect("World path not found for SMP");
        (channel, PathBuf::from(world_path))
    };
    let mut world = World::new(world_path);
    loop {
        let instant = std::time::Instant::now();
        {
            let prints = run_loop(&mut world);
            let duration_since_epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::new(0, 0))
                .as_secs();
            let mut embed = poise::serenity_prelude::CreateEmbed::default()
                .title("Info Board")
                .description(format!("Last updated: <t:{}:R>", duration_since_epoch));
            for print in prints {
                let split = print.split_once('|').unwrap_or((&print, ""));
                let (title, content) = split;
                embed = embed.field(title.trim(), content.trim(), false);
            }


            if let Some(ref mut msg) = message {
                let edit_message = EditMessage::new()
                    .embed(embed);
                msg.edit(&ctx, edit_message).await.ok();
            } else {
                let create_message = CreateMessage::new()
                    .embed(embed);
                message = channel.send_message(&ctx, create_message).await.ok()
            }
        }
        let elapsed = instant.elapsed();
        let sec_45 = std::time::Duration::from_secs(45);
        if elapsed < sec_45 {
            sleep(sec_45 - elapsed).await;
        }
    }

}
