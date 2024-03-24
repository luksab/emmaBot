use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use std::time::Duration;
use tracing::{debug, error};

use crate::{get_numer_of_users_in_channel, State, UserIDGuildID};

pub async fn handle_voice_state_update(ctx: Context, old: Option<VoiceState>, new: VoiceState) {
    debug!("voice_state_update: \nold: {:?} \nnew: {:?}", old, new);
    let channel = match new.channel_id {
        Some(channel_id) => channel_id.to_channel(&ctx.http).await.unwrap(),
        None => old
            .as_ref()
            .unwrap()
            .channel_id
            .unwrap()
            .to_channel(&ctx.http)
            .await
            .unwrap(),
    };

    let channel = channel.guild().unwrap();
    let number_of_users_in_channel = get_numer_of_users_in_channel(&ctx, &new).await;
    // if user joins a voice channel
    if old.is_none() && new.channel_id.is_some() && number_of_users_in_channel == 1 {
        debug!("User joined channel");
        // wait one minute
        tokio::time::sleep(Duration::from_secs(60)).await;
        debug!("Checking if user is still in channel");
        // check if user is still in the channel
        let number_of_users_in_channel = get_numer_of_users_in_channel(&ctx, &new).await;
        if number_of_users_in_channel == 0 {
            // everyone left the channel
            return;
        }
        // add channel to map
        let mut data = ctx.data.write().await;
        let state = data.get_mut::<State>().unwrap();
        state.occupied_channels.insert(new.channel_id.unwrap());
        drop(data);
        let guild_id = new.guild_id.unwrap().0 as i64;

        let to_ping_user_ids: Vec<UserIDGuildID> = sqlx::query_as!(
            UserIDGuildID,
            "SELECT * FROM UserIDGuildID WHERE guild_id = $1",
            guild_id
        )
        .fetch_all(&ctx.data.read().await.get::<State>().unwrap().pool)
        .await
        .unwrap();

        for user_id_guild_id in to_ping_user_ids {
            // check that user is not in the channel
            if user_id_guild_id.user_id == new.user_id.0 as i64 {
                continue;
            }
            let user_id = user_id_guild_id.user_id;
            let user = match ctx.cache.user(user_id as u64) {
                Some(user) => user,
                None => {
                    // get user from api
                    ctx.http.get_user(user_id as u64).await.unwrap()
                }
            };
            let channel_name = new.channel_id.unwrap().name(&ctx.cache).await.unwrap();
            let invite = channel
                .create_invite(&ctx.http, |i| i.max_uses(1))
                .await
                .unwrap();
            if let Err(e) = user
                .direct_message(&ctx.http, |m| {
                    m.add_embed(|e| {
                        e.title(new.guild_id.unwrap().name(&ctx.cache).unwrap())
                            .url(invite.url())
                            .author(|a| {
                                a.name(new.member.as_ref().unwrap().display_name())
                                    // .url("https://discord.gg/invite")
                                    .icon_url(
                                        new.member.as_ref().unwrap().user.avatar_url().unwrap_or(
                                            new.member.as_ref().unwrap().user.default_avatar_url(),
                                        ),
                                    )
                            })
                            .description(format!(
                                "{} Started VC in {}",
                                new.member.as_ref().unwrap().display_name(),
                                channel_name,
                            ))
                            .thumbnail(
                                new.guild_id
                                    .unwrap()
                                    .to_guild_cached(&ctx.cache)
                                    .unwrap()
                                    .icon_url()
                                    .unwrap(),
                            )
                    })
                })
                .await
            {
                error!("Error sending message: {:?}", e);
            }
        }
    } else if new.channel_id.is_none() {
        // if user leaves a voice channel
        if let Some(channel_id) = old.unwrap().channel_id {
            // check that no one is in the channel
            if !channel.members(&ctx.cache).await.unwrap().is_empty() {
                return;
            }
            // remove channel from map
            let mut data = ctx.data.write().await;
            let state = data.get_mut::<State>().unwrap();
            let was_removed = state.occupied_channels.remove(&channel_id);
            if !was_removed {
                return;
            }
            drop(data);
            let guild_id = new.guild_id.unwrap().0 as i64;
            let to_ping_user_ids: Vec<UserIDGuildID> = sqlx::query_as!(
                UserIDGuildID,
                "SELECT * FROM UserIDGuildID WHERE guild_id = $1",
                guild_id
            )
            .fetch_all(&ctx.data.read().await.get::<State>().unwrap().pool)
            .await
            .unwrap();

            for user_id_guild_id in to_ping_user_ids {
                let send_disconnect_message = user_id_guild_id.disconnect_message.unwrap_or(true);
                if !send_disconnect_message {
                    continue;
                }
                let user_id = user_id_guild_id.user_id;
                let user = match ctx.cache.user(user_id as u64) {
                    Some(user) => user,
                    None => {
                        // get user from api
                        ctx.http.get_user(user_id as u64).await.unwrap()
                    }
                };
                // don't send message to user who left
                if user_id == new.user_id.0 as i64 {
                    continue;
                }

                let channel_name = channel_id.name(&ctx.cache).await.unwrap();
                if let Err(e) = user
                    .direct_message(&ctx.http, |m| {
                        m.add_embed(|e| {
                            e.title(new.guild_id.unwrap().name(&ctx.cache).unwrap())
                                .author(|a| {
                                    a.name(new.member.as_ref().unwrap().display_name())
                                        .icon_url(
                                            new.member
                                                .as_ref()
                                                .unwrap()
                                                .user
                                                .avatar_url()
                                                .unwrap_or(
                                                    new.member
                                                        .as_ref()
                                                        .unwrap()
                                                        .user
                                                        .default_avatar_url(),
                                                ),
                                        )
                                })
                                .description(format!(
                                    "{} Stopped VC in {}",
                                    new.member.as_ref().unwrap().display_name(),
                                    channel_name,
                                ))
                                .thumbnail(
                                    new.guild_id
                                        .unwrap()
                                        .to_guild_cached(&ctx.cache)
                                        .unwrap()
                                        .icon_url()
                                        .unwrap(),
                                )
                        })
                    })
                    .await
                {
                    error!("Error sending message: {:?}", e);
                }
            }
        }
    }
}
