pub mod commands;
pub mod messages;
pub mod config;
pub mod voice_state_update;

pub use commands::*;
pub use messages::*;
pub use config::*;
pub use voice_state_update::*;

use serde::{Deserialize, Serialize};
use serenity::model::id::ChannelId;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use sqlx::SqlitePool;
use std::collections::HashSet;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserIDGuildID {
    pub user_id: i64,
    pub guild_id: i64,
    pub disconnect_message: Option<bool>,
}

pub struct State {
    pub pool: SqlitePool,
    pub occupied_channels: HashSet<ChannelId>,
}

impl TypeMapKey for State {
    type Value = State;
}

pub async fn get_numer_of_users_in_channel(ctx: &Context, state: &VoiceState) -> usize {
    let channel = match state.channel_id {
        Some(channel_id) => channel_id.to_channel(&ctx.http).await.unwrap(),
        None => return 0,
    }
    .guild()
    .unwrap();
    match state.channel_id {
        Some(channel_id) => match channel.members(&ctx.cache).await {
            Ok(members) => members.len(),
            Err(_) => {
                // get members from api
                let channel = channel_id.to_channel(&ctx.http).await.unwrap();
                let channel = channel.guild().unwrap();
                let members = channel.members(&ctx.cache).await.unwrap();
                members.len()
            }
        },
        None => 0,
    }
}
