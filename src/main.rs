pub mod anvil;
pub mod commands;
pub mod config;
pub mod scoreboard;
pub mod taurus;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::env;
use std::fs::{File, read_to_string};
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Once};

use flate2::bufread::GzDecoder;
use futures::lock::Mutex;
use http::Uri;
use poise::Prefix;
use poise::samples::create_application_commands;
use poise::serenity_prelude::futures::{SinkExt, StreamExt};
use poise::serenity_prelude::prelude::TypeMapKey;
use poise::serenity_prelude::{
    ChannelId, Client, Command, Context, EventHandler, GatewayIntents, Message, Ready, async_trait,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_websockets::{
    ClientBuilder, Error, MaybeTlsStream, Message as WSMessage, WebSocketStream,
};
use valence_nbt::{Value, from_binary};

use crate::anvil::run_anvil;
use crate::commands::{member, public};
use crate::config::{Config, ConfigValue};
use crate::scoreboard::{CachedScoreboard, Scoreboards};
use crate::taurus::{TaurusChannel, send_message, taurus_connection};

#[derive(Debug)]
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Check if the message is from a bot or not
        if msg.author.bot {
            return; // Ignore messages from bots
        }
        {
            let data = ctx.data.read().await;
            let id = data
                .get::<Config>()
                .expect("TaurusChannel not found")
                .chat_bridge;

            if msg.channel_id != ChannelId::new(id) {
                return; // Ignore messages not in the chat bridge channel
            }

            let (tx, _rx) = data
                .get::<TaurusChannel>()
                .expect("TaurusChannel not found");

            send_message(msg, tx).await;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        static START_CHILD_THREADS: Once = Once::new();
        let mut data = ctx.data.write().await;
        START_CHILD_THREADS.call_once(|| {
            let (tx, rx) = tokio::sync::mpsc::channel(100);
            let command_responses = Arc::new(Mutex::new(Vec::new()));
            {
                // Create a channel for Taurus messages

                data.insert::<TaurusChannel>((tx, command_responses.clone()));
            }
            let taurus_ctx = ctx.clone();
            tokio::spawn(async move {
                taurus_connection(&taurus_ctx, rx, command_responses).await;
            });
            let anvil_ctx = ctx.clone();
            tokio::spawn(async move {
                run_anvil(&anvil_ctx).await;
            });
            println!("INFO: Started child threads");
        });
        println!("INFO: {} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let config_json = read_to_string("config.json").expect("Failed to read config.json");
    let config: ConfigValue =
        serde_json::from_str(&config_json).expect("Failed to parse config.json");

    // Login with a bot token from the environment
    let token = env::var("API_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                public::age(),
                public::hardware(),
                public::score(),
                public::iscore(),
                public::list(),
                public::invite(),
                member::backup(),
                member::grinder(),
                member::session(),
                member::reconnect(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: config.prefix.first().cloned(),
                ..Default::default()
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let commands = &framework.options().commands;
                poise::builtins::register_globally(ctx, commands)
                    .await
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

    {
        let mut data = client.data.write().await;
        let scoreboard_path = PathBuf::from(
            config
                .get_world_path("SMP")
                .expect("Failed to get world path"),
        );
        // Insert the chat bridge URL into the data
        data.insert::<Config>(config);

        let cached_scoreboard = CachedScoreboard::new(scoreboard_path);
        data.insert::<Scoreboards>(cached_scoreboard);
    }

    println!("INFO: Connecting to Discord...");
    // Start the client
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
