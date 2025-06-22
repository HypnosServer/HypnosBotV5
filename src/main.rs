pub mod commands;

use std::env;
use std::fs::{read_to_string, File};
use std::io::{BufReader, Read, Write};


use flate2::bufread::GzDecoder;
use http::Uri;
use poise::samples::create_application_commands;
use poise::serenity_prelude::futures::{SinkExt, StreamExt};
use poise::serenity_prelude::prelude::TypeMapKey;
use poise::serenity_prelude::{async_trait, ChannelId, Client, Command, Context, EventHandler, GatewayIntents, Message, Ready};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_websockets::{ClientBuilder, Error, MaybeTlsStream, Message as WSMessage, WebSocketStream};
use valence_nbt::{from_binary, Value};

use crate::commands::public;


struct TaurusChannel;

impl TypeMapKey for TaurusChannel {
    type Value = Sender<String>;
}

#[derive(Debug, Clone, Deserialize)]
struct World {
    name: String,
    path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbedOpts {
    colour: String,
    footer_text: String,
    footer_icon_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigValue {
    name: String,
    prefix: Vec<String>,
    staff: Vec<u64>,
    admin_role: u64,
    member_role: u64,
    grinder_role: u64,
    worlds: Vec<World>,
    chat_bridge: u64,
    embed_opts: EmbedOpts,
}

struct Config;

impl TypeMapKey for Config {
    type Value = ConfigValue;
}

struct Scoreboards;

impl TypeMapKey for Scoreboards {
    type Value = Vec<String>;
}

#[derive(Debug)]
struct Handler;

async fn auth_taurus(ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> Result<(), ()>
{
    let password = env::var("TAURUS_PASS")
        .expect("Expected a TAURUS_PASS environment variable");

    ws.send(WSMessage::text(password)).await.or(Err(()))?;
    ws.send(WSMessage::text("PING")).await.or(Err(()))?;
    let Some(Ok(msg)) = ws.next().await else {
        println!("Failed to receive authentication response from Taurus");
        return Err(());
    };
    if msg.as_text().unwrap_or("") != "PONG" {
        println!("Authentication failed: expected 'PONG', got {:?}", msg);
    }

    Ok(())
}

fn split_incoming_msg<'a>(msg: &'a WSMessage) -> Option<(&'a str, &'a str)> {
    let split = msg.as_text()?.split_once(" ");
    split
}

async fn print_to_discord(channel: &ChannelId, ctx: &Context, msg: WSMessage) {
    let (_command, content) = split_incoming_msg(&msg)
        .expect("Failed to split incoming message");
    channel.say(&ctx.http, content).await.expect("Failed to send message to Discord");
}


async fn chat_bridge(ctx: &Context, mut rx: Receiver<String>) {
    let uri = Uri::from_static("ws://127.0.0.1:9000/taurus");
    let (mut ws, _res) = ClientBuilder::from_uri(uri)
        .connect()
        .await
        .expect("Failed to connect to Taurus WebSocket");
    let channel = {
        let data = ctx.data.read().await;
        let id = data.get::<Config>()
            .expect("ChatBridge not found")
            .chat_bridge;
        
        ChannelId::new(id)
    };

    auth_taurus(&mut ws).await.expect("Failed to authenticate with Taurus");
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                ws.send(WSMessage::text(msg)).await
                    .expect("Failed to send message to Taurus");
            },
            Some(Ok(msg)) = ws.next() => {
                print_to_discord(&channel, ctx, msg).await;
            }

        }
    }
}

fn mc_format(msg: &str, color: &[char]) -> String {
    let mut formatted = String::new();
    for c in color {
        formatted.push('§');
        formatted.push(*c);
    }
    formatted.push_str(msg);
    formatted.push_str("§r");
    formatted
}

async fn send_message(msg: Message, tx: &Sender<String>) {
    let author_name = msg.author.name;
    let content = msg.content;
    let replying_to = msg.referenced_message;
    let mut message = String::from("MSG ");
    if let Some(reply) = replying_to {
        message.push_str(&format!("reply -> {} {}\n", mc_format(&reply.author.name, &['d']), reply.content));
    }
    message.push_str(&format!("[{}] {}", mc_format(&author_name, &['5']), content));
    tx.send(message)
        .await
        .expect("Failed to send message to Taurus channel");

    let has_attachments = !msg.attachments.is_empty();
    if has_attachments {
        let text = if msg.attachments.len() == 1 {
            "Attachment".to_string()
        } else {
            format!("Attachments ({})", msg.attachments.len())
        };
        tx.send(format!("MSG [{}] {}", mc_format(&author_name, &['5']), text))
            .await
            .expect("Failed to send attachment message to Taurus channel");
        for attachment in msg.attachments {
            let url = attachment.url;
            let filename = attachment.filename;
            tx.send(format!("URL {} {}", url, mc_format(&filename, &['9', 'n'])))
                .await
                .expect("Failed to send attachment URL to Taurus channel");
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Check if the message is from a bot or not
        if msg.author.bot {
            return; // Ignore messages from bots
        }
        {
            let data = ctx.data.read().await;
            let id = data.get::<Config>()
                .expect("TaurusChannel not found")
                .chat_bridge;

            if msg.channel_id != ChannelId::new(id) {
                return; // Ignore messages not in the chat bridge channel
            }

            let tx = data.get::<TaurusChannel>()
                .expect("TaurusChannel not found");

            send_message(msg, &tx).await;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        {
            let mut data = ctx.data.write().await;
            // Create a channel for Taurus messages
    
            data.insert::<TaurusChannel>(tx);
        }
        // Start the chat bridge in a separate task
        tokio::spawn(async move {
            chat_bridge(&ctx, rx).await;
        });
    }
}

fn read_scoreboard_to_nbt() -> Option<Vec<String>> {
    let mut file = File::open("data/scoreboard.dat").ok()?;
    let mut buf = Vec::new();
    let mut d = GzDecoder::new(BufReader::new(&mut file));
    d.read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read score file: {}", e)).ok()?;
    let (scoreboard, _) = from_binary::<String>(&mut buf.as_slice())
        .map_err(|e| format!("Failed to parse score file: {}", e)).ok()?;

    let Some(Value::Compound(data)) = scoreboard.get("data") else {
        return None;
    };
    let Some(Value::List(objectives)) = data.get("Objectives") else {
        return None;
    };
    let objectives: Vec<String> = objectives.iter()
        .filter_map(|objective| {
            if let Value::Compound(compound) = objective.to_value() {
                compound.get("Name").and_then(|name| {
                    if let Value::String(name_str) = name {
                        Some(name_str.to_string())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
        .collect();
    println!("Objectives: {:?}", objectives);
    return Some(objectives);
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let scoreboard_dat = include_bytes!("../data/scoreboard.dat");
    let mut buf = Vec::new();
    let mut gz = GzDecoder::new(scoreboard_dat.as_ref());
    gz.read_to_end(&mut buf)
        .expect("Failed to decompress scoreboard.dat");
    let (scoreboard, _) = valence_nbt::from_binary::<String>(&mut buf.as_slice())
        .expect("Failed to parse scoreboard.dat");
    println!("Scoreboard: {:#?}", scoreboard);


    let config_json = read_to_string("config.json")
        .expect("Failed to read config.json");
    
    // Login with a bot token from the environment
    let token = env::var("API_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                public::age(), public::hardware(), public::score(),
                ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some(";".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let commands = &framework.options().commands;
                poise::builtins::register_globally(ctx, commands).await
                    .expect("Failed to register commands globally");

                let commands = create_application_commands(commands);

                Command::set_global_commands(ctx, commands)
                    .await
                    .expect("Failed to set global commands");
                
                // This is where you can initialize your data
                Ok(())
            })
        })
        .build();
    
    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");
    let config: ConfigValue = serde_json::from_str(&config_json).expect("Failed to parse config.json");
    let scoreboards = read_scoreboard_to_nbt().unwrap_or_else(|| {
        println!("No scoreboards found, using empty list");
        vec![]
    });

    {
        let mut data = client.data.write().await;
        // Insert the chat bridge URL into the data
        data.insert::<Config>(config);
        data.insert::<Scoreboards>(scoreboards);
    }

    // Start the client
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}