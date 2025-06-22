pub mod commands;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::env;
use std::fs::{read_to_string, File};
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::Arc;


use flate2::bufread::GzDecoder;
use futures::lock::Mutex;
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
    type Value = (Sender<String>, Arc<Mutex<Vec<String>>>);
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

impl ConfigValue {
    pub fn get_world_path(&self, world_name: &str) -> Option<String> {
        self.worlds.iter()
            .find(|world| world.name == world_name)
            .map(|world| world.path.clone())
    }
}

struct Config;

impl TypeMapKey for Config {
    type Value = ConfigValue;
}

pub struct ScoreboardNames {
    pub names: Vec<String>,
    last_update: std::time::Instant,
}

impl ScoreboardNames {
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            last_update: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, names: Vec<String>) {
        self.names = names;
        self.last_update = std::time::Instant::now();
    }

    pub fn should_update(&self) -> bool {
        self.last_update.elapsed().as_secs() > 60 // 1 minute
    }
}

#[derive(Debug, Clone)]
pub struct Scoreboard {
    pub name: String,
    pub scores: HashMap<String, i32>,
    pub total: i64,
    last_update: std::time::Instant,
}

impl Scoreboard {
    pub fn new(name: String) -> Self {
        Self {
            name,
            scores: HashMap::new(),
            total: 0,
            last_update: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, scores: HashMap<String, i32>, total: i64) {
        self.scores = scores;
        self.total = total;
        self.last_update = std::time::Instant::now();
    }

    pub fn should_update(&self) -> bool {
        self.last_update.elapsed().as_secs() > 60 // 1 minute
    }
}

pub struct CachedScoreboard {
    pub scoreboard_names: ScoreboardNames,
    pub scoreboards: HashMap<String, Scoreboard>,
    // Bucket to delete scoreboards that are not used anymore

    path: PathBuf,
}

impl CachedScoreboard {
    pub fn new(path: PathBuf) -> Self {
        let mut s = Self {
            scoreboard_names: ScoreboardNames::new(),
            scoreboards: HashMap::new(),
            path,
        };
        s.load_names()
            .unwrap_or_else(|e| println!("Failed to load scoreboard names: {}", e));
        s
    }

    pub fn load_names(&mut self) -> Result<(), String> {
        let mut file = File::open(&self.path).map_err(|e| format!("Failed to open scoreboard file: {}", e))?;
        let mut buf = Vec::new();
        let mut d = GzDecoder::new(BufReader::new(&mut file));
        d.read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read score file: {}", e))?;
        let (scoreboard, _) = from_binary::<String>(&mut buf.as_slice())
            .map_err(|e| format!("Failed to parse score file: {}", e))?;

        let Some(Value::Compound(data)) = scoreboard.get("data") else {
            return Err("No data found in scoreboard".to_string());
        };
        let Some(Value::List(objectives)) = data.get("Objectives") else {
            return Err("No objectives found in scoreboard".to_string());
        };
        let names = objectives.iter()
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
            .collect::<Vec<String>>();
        self.scoreboard_names.update(names);

        Ok(())
    }

    pub fn load_scoreboard(
        &mut self,
        name: &str,
    ) -> Result<(), String> {
        let mut file = File::open(&self.path).map_err(|e| format!("Failed to open scoreboard file: {}", e))?;
        let mut buf = Vec::new();
        let mut d = GzDecoder::new(BufReader::new(&mut file));
        d.read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read score file: {}", e))?;
        let (scoreboard, _) = from_binary::<String>(&mut buf.as_slice())
            .map_err(|e| format!("Failed to parse score file: {}", e))?;

        let Some(Value::Compound(data)) = scoreboard.get("data") else {
            return Err("No data found in scoreboard".to_string());
        };

        let Some(Value::List(player_scores)) = data.get("PlayerScores") else {
            return Err("No player scores found in scoreboard".to_string());
        };

        let mut scores = HashMap::new();
        let mut total = 0;
        for score in player_scores.iter() {
            if let Value::Compound(compound) = score.to_value() {
                if let Some(Value::String(objective_name)) = compound.get("Objective") {
                    if objective_name == name {
                        if let Some(Value::String(player_name)) = compound.get("Name") {
                            if player_name == "Total" {
                                continue;
                            }
                            if let Some(Value::Int(score_value)) = compound.get("Score") {
                                scores.insert(player_name.to_string(), *score_value);
                                total += *score_value as i64;
                            }
                        }
                    }
                }
            }
        }

        if scores.is_empty() {
            return Err(format!("No scores found for objective '{}'", name));
        }

        match self.scoreboards.entry(name.to_string()) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().update(scores, total);
            }
            Entry::Vacant(entry) => {
                let scoreboard = Scoreboard::new(name.to_string());
                entry.insert(scoreboard).update(scores, total);
            }
        }
        
        Ok(())
    }

    pub fn get_scoreboard(
        &mut self,
        name: &str,
    ) -> Result<&Scoreboard, String> {
        if self.scoreboards.get(name).is_none() {
            self.load_scoreboard(name)?;
        }
        self.scoreboards.get(name).ok_or_else(|| format!("Scoreboard '{}' not found", name))
    }

}

struct Scoreboards;

impl TypeMapKey for Scoreboards {
    type Value = CachedScoreboard;
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

fn is_bridge(msg: &WSMessage) -> bool {
    if let Some((command, _)) = split_incoming_msg(msg) {
        command == "MSG"
    } else {
        false
    }
}

async fn taurus_connection(ctx: &Context, mut rx: Receiver<String>, mut command_responses: Arc<Mutex<Vec<String>>>) {
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
                if is_bridge(&msg) {
                    print_to_discord(&channel, ctx, msg).await;
                } else {
                    let string = msg.as_text().unwrap();
                    command_responses.lock().await.push(string.to_string());
                }
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


pub async fn fetch_latest_with_type(
    message_cache: Arc<Mutex<Vec<String>>>,
    ty: &str,
) -> Result<String, String> {
    let mut tries = 0;
    loop {
        let mut cache = message_cache.lock().await;
        if !cache.is_empty() {
            if let Some(latest) = cache.last() {
                if latest.starts_with(ty) {
                    // Pop the latest message from the cache
                    if let Some(message) = cache.pop() {
                        return Ok(message);
                    } else {
                        return Err("Failed to pop message from cache".to_string());
                    }
                }
            }
        }
        tries += 1;
        if tries > 5 {
            return Err("Timeout".to_string());
        }
        drop(cache); // Release the lock before sleeping
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
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

            let (tx, _rx) = data.get::<TaurusChannel>()
                .expect("TaurusChannel not found");

            send_message(msg, &tx).await;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let command_responses = Arc::new(Mutex::new(Vec::new()));
        {
            let mut data = ctx.data.write().await;
            // Create a channel for Taurus messages
    
            data.insert::<TaurusChannel>((tx, command_responses.clone()));
        }
        // Start the chat bridge in a separate task
        tokio::spawn(async move {
            taurus_connection(&ctx, rx, command_responses).await;
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
                public::age(), public::hardware(), public::score(), public::list(), public::invite(),
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
        let scoreboard_path = PathBuf::from("data/scoreboard.dat");
        let cached_scoreboard = CachedScoreboard::new(scoreboard_path);
        data.insert::<Scoreboards>(cached_scoreboard);
    }

    // Start the client
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}