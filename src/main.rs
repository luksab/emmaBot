use std::collections::HashSet;
use std::time::Duration;

use crate::application::interaction::{Interaction, InteractionResponseType};
use config::Config;
use lukas_bot::get_numer_of_users_in_channel;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serenity::model::application::command::{Command, CommandOptionType};
use serenity::model::prelude::ChannelId;
use serenity::model::{channel::Message, gateway::Ready, voice::VoiceState};
use serenity::prelude::*;
use serenity::{async_trait, model::application};
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Sqlite, SqlitePool};
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::prelude::*;

mod config;

struct Bot;

#[derive(Debug, Serialize, Deserialize)]
struct UserIDGuildID {
    user_id: i64,
    guild_id: i64,
    disconnect_message: Option<bool>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        // check that I didn't sent the message
        if msg.author.bot {
            return;
        }
        if msg.author.id
            == std::env::var("OWNER_ID")
                .unwrap()
                .parse::<serenity::model::id::UserId>()
                .unwrap()
        {
            if msg.content == "$export" {
                let data = ctx.data.read().await;
                let state = data.get::<State>().unwrap();
                let pool = &state.pool;
                // read all user_id_guild_id
                let user_id_guild_id: Vec<UserIDGuildID> =
                    sqlx::query_as!(UserIDGuildID, "SELECT * FROM UserIDGuildID")
                        .fetch_all(pool)
                        .await
                        .unwrap();
                // export to json
                let json = serde_json::to_string(&user_id_guild_id).unwrap();
                // send json to user
                msg.channel_id.say(&ctx.http, json).await.unwrap();
            } else if msg.content.starts_with("$import") {
                let data = ctx.data.read().await;
                let state = data.get::<State>().unwrap();
                let pool = &state.pool;
                let json = msg.content.replace("$import", "");
                let user_id_guild_id: Vec<UserIDGuildID> =
                    serde_json::from_str(&json).expect("Failed to parse json");
                for user_id_guild_id in &user_id_guild_id {
                    sqlx::query!(
                        "INSERT INTO UserIDGuildID (user_id, guild_id, disconnect_message) VALUES ($1, $2, $3)",
                        user_id_guild_id.user_id,
                        user_id_guild_id.guild_id,
                        user_id_guild_id.disconnect_message
                    )
                    .execute(pool)
                    .await
                    .unwrap();
                }
                msg.channel_id
                    .say(
                        &ctx.http,
                        format!("Imported {} rows", user_id_guild_id.len()),
                    )
                    .await
                    .unwrap();
            }
        }
        let jokes = ctx.data.read().await.get::<Config>().unwrap().jokes.clone();

        for joke in jokes {
            let matches = joke.regex.captures(&msg.content).unwrap();
            if let Some(matches) = matches {
                // replace --[number]-- with the corresponding capture group
                let mut message = joke
                    .message
                    .choose(&mut rand::thread_rng())
                    .unwrap()
                    .to_owned();
                for (i, capture) in matches.iter().enumerate() {
                    if let Some(capture) = capture {
                        message = message.replace(&format!("--[{}]--", i), capture.as_str());
                    }
                }
                // replace --[nickname]-- with the nickname of the user
                let nickname = msg
                    .guild_id
                    .unwrap()
                    .member(&ctx.http, msg.author.id)
                    .await
                    .unwrap()
                    .nick
                    .unwrap_or(msg.author.name.clone());
                message = message.replace("--[nickname]--", &nickname);
                // replace --[username]-- with the username of the user
                message = message.replace("--[username]--", &msg.author.name);
                // replace --[guild]-- with the name of the guild
                let guild = msg
                    .guild_id
                    .unwrap()
                    .to_partial_guild(&ctx.http)
                    .await
                    .unwrap();
                message = message.replace("--[guild]--", &guild.name);
                // replace --[channel]-- with the name of the channel
                let channel = msg.channel_id.to_channel(&ctx.http).await.unwrap();
                message = message.replace(
                    "--[channel]--",
                    &channel.guild().map(|c| c.name).unwrap_or("DM".to_string()),
                );
                msg.channel_id.say(&ctx.http, message).await.unwrap();
                break;
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let user_id = command.member.as_ref().unwrap().user.id.0 as i64;
            let message_text = match command.data.name.as_str() {
                "vcping" => {
                    // get command options
                    let disconnect_message = command
                        .data
                        .options
                        .iter()
                        .find(|option| option.name == "disconnect-message")
                        .and_then(|option| option.clone().value.unwrap().as_bool());
                    let guild_id = command.guild_id.unwrap().0 as i64;
                    // let guild = guild_id.to_partial_guild(&ctx.http).await.unwrap();
                    let user_id_exists: Option<UserIDGuildID> = sqlx::query_as!(
                        UserIDGuildID,
                        "SELECT * FROM UserIDGuildID WHERE user_id = $1 AND guild_id = $2",
                        user_id,
                        guild_id
                    )
                    .fetch_optional(&ctx.data.read().await.get::<State>().unwrap().pool)
                    .await
                    .unwrap();

                    // add user to map, if they are not already in it
                    // remove user from map, if they are already in it
                    if user_id_exists.is_none() {
                        sqlx::query!(
                            "INSERT INTO UserIDGuildID (user_id, guild_id, disconnect_message) VALUES ($1, $2, $3)",
                            user_id,
                            guild_id,
                            disconnect_message
                        )
                        .execute(&ctx.data.read().await.get::<State>().unwrap().pool)
                        .await
                        .unwrap();

                        "You have been added to the ping list!"
                    } else {
                        // check, if the user wants to change the disconnect message setting
                        if let Some(disconnect_message) = disconnect_message {
                            sqlx::query!(
                                "UPDATE UserIDGuildID SET disconnect_message = $1 WHERE user_id = $2 AND guild_id = $3",
                                disconnect_message,
                                user_id,
                                guild_id
                            )
                            .execute(&ctx.data.read().await.get::<State>().unwrap().pool)
                            .await
                            .unwrap();
                            "Your disconnect message setting has been updated!"
                        } else {
                            // remove user from map
                            sqlx::query!(
                                "DELETE FROM UserIDGuildID WHERE user_id = $1 AND guild_id = $2",
                                user_id,
                                guild_id
                            )
                            .execute(&ctx.data.read().await.get::<State>().unwrap().pool)
                            .await
                            .unwrap();
                            // send message to user
                            "You have been removed from the ping list!"
                        }
                    }
                }

                command => unreachable!("Unknown command: {}", command),
            };

            command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.content(message_text))
                })
                .await
                .unwrap();
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
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
                    let send_disconnect_message =
                        user_id_guild_id.disconnect_message.unwrap_or(true);
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

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        // register commands
        let commands = Command::set_global_application_commands(&ctx.http, |commands| {
            commands.create_application_command(|command| {
                command
                    .name("vcping")
                    .description(
                        "Get added to the list of UserIDGuildID to ping when someone starts a VC",
                    )
                    .create_option(|option| {
                        option
                            .name("disconnect-message")
                            .description("Also send a message when someone disconnects from VC")
                            .kind(CommandOptionType::Boolean)
                            .required(false)
                    })
            })
        })
        .await
        .unwrap();

        info!("Slash commands registered: {:?}", commands);
    }
}

struct State {
    pool: SqlitePool,
    occupied_channels: HashSet<ChannelId>,
}

impl TypeMapKey for State {
    type Value = State;
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let filter = tracing_subscriber::filter::Targets::new()
        .with_default(Level::DEBUG)
        .with_target("sqlx", Level::WARN)
        .with_target("h2", Level::WARN)
        .with_target("hyper", Level::WARN)
        .with_target("reqwest", Level::WARN)
        .with_target("serenity", Level::WARN)
        .with_target("tokio", Level::WARN)
        .with_target("rustls", Level::WARN);

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::Layer::default())
        .init();

    info!("Starting bot");

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    println!("Database url: {}", db_url);
    // print pwd
    println!("Current directory: {:?}", std::env::current_dir().unwrap());
    // make a sqlx sqlite pool
    if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        warn!("Creating database {}", db_url);
        match Sqlite::create_database(&db_url).await {
            Ok(_) => info!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    } else {
        info!("Database already exists");
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("Error connecting to database");

    sqlx::migrate!().run(&pool).await.unwrap();

    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILDS
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES;

    let state = State {
        pool,
        occupied_channels: HashSet::new(),
    };
    let config = config::load_config();

    let mut client = Client::builder(&token, intents)
        .event_handler(Bot)
        .type_map_insert::<State>(state)
        .type_map_insert::<Config>(config)
        .await
        .expect("Err creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
