use poise::serenity_prelude::prelude::TypeMapKey;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct World {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbedOpts {
    pub colour: String,
    pub footer_text: String,
    pub footer_icon_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValue {
    pub name: String,
    pub prefix: Vec<String>,
    pub staff: Vec<u64>,
    pub admin_role: u64,
    pub member_role: u64,
    pub grinder_role: u64,
    pub worlds: Vec<World>,
    pub chat_bridge: u64,
    pub info_channel: u64,
    pub embed_opts: EmbedOpts,
}

impl ConfigValue {
    pub fn get_world_path(&self, world_name: &str) -> Option<String> {
        self.worlds
            .iter()
            .find(|world| world.name == world_name)
            .map(|world| world.path.clone())
    }
}

pub struct Config;

impl TypeMapKey for Config {
    type Value = ConfigValue;
}
