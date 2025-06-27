use std::sync::Arc;

use futures::lock::Mutex;
use poise::serenity_prelude::prelude::TypeMapKey;
use tokio::sync::mpsc::Sender;

pub struct TaurusChannel;

impl TypeMapKey for TaurusChannel {
    type Value = (Sender<String>, Arc<Mutex<Vec<String>>>);
}
