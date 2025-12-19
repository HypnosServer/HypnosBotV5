use std::{env, str::FromStr, sync::Arc};

use futures::{lock::Mutex, FutureExt, SinkExt, StreamExt};
use http::Uri;
use poise::serenity_prelude::{prelude::TypeMapKey, ChannelId, Context, Message};
use tokio::{net::TcpStream, sync::mpsc::{Receiver, Sender}};
use tokio_websockets::{ClientBuilder, MaybeTlsStream, Message as WSMessage, WebSocketStream};

use crate::{commands::ingame::execute_ingame_command, config::Config};

pub struct TaurusChannel;

impl TypeMapKey for TaurusChannel {
    type Value = (Sender<String>, Arc<Mutex<Vec<String>>>);
}

async fn auth_taurus(ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> Result<(), ()> {
    let Ok(password) = env::var("TAURUS_PASS") else {
        println!("INFO: TAURUS_PASS environment variable not set, skipping authentication");
        return Ok(());
    };

    ws.send(WSMessage::text(password)).await.or(Err(()))?;
    ws.send(WSMessage::text("PING")).await.or(Err(()))?;
    let Some(Ok(msg)) = ws.next().await else {
        println!("INFO: Failed to receive authentication response from Taurus");
        return Err(());
    };
    if msg.as_text().unwrap_or("").split(" ").next() != Some("PONG") {
        println!("INFO: Authentication failed: expected 'PONG'");
    }

    Ok(())
}

fn split_incoming_msg<'a>(msg: &'a WSMessage) -> Option<(&'a str, &'a str)> {
    let split = msg.as_text()?.split_once(" ");
    split
}

fn get_body<'a>(msg: &'a WSMessage) -> Option<&'a str> {
    let (command, message) = split_incoming_msg(msg).expect("Failed to split incoming message");
    if command != "MSG" {
        return None;
    }
    let end_username_index = message.chars().position(|c| c == '>')?;
    Some(&message[end_username_index+2..])
}

async fn print_to_discord(channel: &ChannelId, ctx: &Context, msg: WSMessage) {
    let (_command, content) = split_incoming_msg(&msg).expect("Failed to split incoming message");
    channel
        .say(&ctx.http, content)
        .await
        .expect("Failed to send message to Discord");
}

fn is_bridge(msg: &WSMessage) -> bool {
    if let Some((command, _)) = split_incoming_msg(msg) {
        command == "MSG"
    } else {
        false
    }
}

async fn connect(uri_str: &str) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, ()> {
    let Ok(uri) = Uri::from_str(uri_str) else {
        println!("ERROR: Invalid TAURUS_URL: {}", uri_str);
        return Err(());
    };
    let Ok((mut ws, _res)) = ClientBuilder::from_uri(uri)
        .connect()
        .await else {
            println!("ERROR: Failed to connect to Taurus at {}", uri_str);
            return Err(());
        };
    println!("INFO: Connected to Taurus at {}", uri_str);
    println!("INFO: Authenticating with Taurus...");
    if let Err(_) = auth_taurus(&mut ws).await {
        println!("ERROR: Failed to authenticate with Taurus");
        return Err(());
    }
    Ok(ws)
}

fn parse_server(msg: &str) -> Option<(&str, &str, &str)> {
    let input = msg.strip_prefix('[')?;
    let (server, rest) = input.split_once(']')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('<')?;
    let (username, rest) = rest.split_once('>')?;

    let message = rest.trim_start();

    Some((server, username, message))
}

fn ingame_command<'a>(prefixes: &'a Vec<String>, msg: &'a WSMessage) -> Option<(&'a str, &'a str, &'a str, Vec<&'a str>)> {
    let (_command, msg) = split_incoming_msg(msg)?;
    let (server, username, msg) = parse_server(msg)?;
    if msg.starts_with('=') {
        let len = 1;
        let split = msg[len..].split(" ");
        let cmd = "eval";
        let args = split.collect();
        return Some((server, username, cmd, args));
    }
    for prefix in prefixes {
        if !msg.starts_with(prefix) {
            continue;
        }
        let len = prefix.len();
        let mut split = msg[len..].split(" ");
        let cmd = split.next()?;
        let args = split.collect();
        return Some((server, username, cmd, args));
    }
    None
}

pub async fn taurus_connection(
    ctx: &Context,
    mut rx: Receiver<String>,
    command_responses: Arc<Mutex<Vec<String>>>,
) {
    let taurus_url = env::var("TAURUS_URL")
        .expect("Expected a TAURUS_URL environment variable");
    let mut taurus_connection = if let Ok(taurus) = connect(&taurus_url).await {
        Some(taurus)
    } else {
        None
    };
    let channel = {
        let data = ctx.data.read().await;
        let id = data
            .get::<Config>()
            .expect("ChatBridge not found")
            .chat_bridge;

        ChannelId::new(id)
    };
    let cmd_prefix = {
        let data = ctx.data.read().await;
        let config = data.get::<Config>().expect("Could not find Config");
        config.prefix.clone()
    };
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                if msg.trim() == "__RECONNECT__" {
                    println!("INFO: Reconnecting to Taurus...");
                    taurus_connection = connect(&taurus_url).await.ok();
                } else if let None = taurus_connection {
                    println!("INFO: Establishing new connection to Taurus...");
                    taurus_connection = connect(&taurus_url).await.ok();
                }
                if let Some(ws) = taurus_connection.as_mut() {
                    if let Err(e) = ws.send(WSMessage::text(msg)).await {
                        println!("ERROR: Failed to send message to Taurus: {}", e);
                        taurus_connection = None; // Reset connection on error
                    }
                } else {
                    println!("ERROR: No active connection to Taurus, cannot send message");
                }
            },
            msg = match taurus_connection.as_mut() {
                    Some(ws) => ws.next().boxed(),
                    None => futures::future::pending().boxed(),
            } => {
                match msg {
                    Some(Ok(msg)) => {
                        if is_bridge(&msg) {
                            if let Some((server, username, cmd, args)) = ingame_command(&cmd_prefix, &msg) {
                                execute_ingame_command(ctx, server, username, cmd, &args).await;
                            }
                            print_to_discord(&channel, ctx, msg).await;
                        } else {
                            let string = msg.as_text().unwrap();
                            command_responses.lock().await.push(string.to_string());
                        }
                    },
                    Some(Err(e)) => {
                        println!("ERROR: Failed to receive message from Taurus: {}", e);
                        taurus_connection = None; // Reset connection on error
                    }
                    None => {
                        println!("INFO: Taurus connection closed");
                        taurus_connection = None; // Reset connection on close
                    }
                }
            }

        }
    }
}

pub fn mc_format(msg: &str, color: &[char]) -> String {
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

pub async fn send_message(msg: Message, tx: &Sender<String>) {
    let author_name = msg.author.name;
    let content = msg.content;
    let replying_to = msg.referenced_message;
    let mut message = String::from("MSG ");
    if let Some(reply) = replying_to {
        message.push_str(&format!(
            "reply -> {} {}\n",
            mc_format(&reply.author.name, &['d']),
            mc_format(&reply.content, &['o'])
        ));
    }
    message.push_str(&format!(
        "[{}] {}",
        mc_format(&author_name, &['5']),
        content
    ));
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
        tx.send(format!(
            "MSG [{}] {}",
            mc_format(&author_name, &['5']),
            text
        ))
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
