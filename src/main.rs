use crate::application::interaction::{Interaction, InteractionResponseType};
use config::Config;
use rand::seq::SliceRandom;
use serenity::model::application::command::Command;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use serenity::{async_trait, model::application};
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Executor, Sqlite, SqlitePool};
use tracing::{error, info, warn, Level};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

mod config;

struct Bot;

#[derive(sqlx::FromRow, Debug)]
struct UserIDGuildID {
    user_id: i64,
    _guild_id: i64,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        // check that I didn't sent the message
        if msg.author.bot {
            return;
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
            let user_id = command.member.as_ref().unwrap().user.id;
            let message_text = match command.data.name.as_str() {
                "vcping" => {
                    let guild_id = command.guild_id.unwrap();
                    // let guild = guild_id.to_partial_guild(&ctx.http).await.unwrap();
                    let user_id_exists: Option<UserIDGuildID> = sqlx::query_as(
                        "SELECT * FROM UserIDGuildID WHERE user_id = $1 AND guild_id = $2",
                    )
                    .bind(user_id.0 as i64)
                    .bind(guild_id.0 as i64)
                    .fetch_optional(&ctx.data.read().await.get::<State>().unwrap().pool)
                    .await
                    .unwrap();

                    // add user to map, if they are not already in it
                    // remove user from map, if they are already in it
                    if user_id_exists.is_none() {
                        sqlx::query(
                            "INSERT INTO UserIDGuildID (user_id, guild_id) VALUES ($1, $2)",
                        )
                        .bind(user_id.0 as i64)
                        .bind(guild_id.0 as i64)
                        .execute(&ctx.data.read().await.get::<State>().unwrap().pool)
                        .await
                        .unwrap();

                        "You have been added to the ping list!"
                    } else {
                        sqlx::query(
                            "DELETE FROM UserIDGuildID WHERE user_id = $1 AND guild_id = $2",
                        )
                        .bind(user_id.0 as i64)
                        .bind(guild_id.0 as i64)
                        .execute(&ctx.data.read().await.get::<State>().unwrap().pool)
                        .await
                        .unwrap();
                        // send message to user
                        "You have been removed from the ping list!"
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
        let number_of_users_in_channel = match new.channel_id {
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
        };
        // if user joins a voice channel
        if old.is_none() && new.channel_id.is_some() && number_of_users_in_channel == 1 {
            let guild_id = new.guild_id.unwrap();

            let to_ping_user_ids: Vec<UserIDGuildID> =
                sqlx::query_as("SELECT * FROM UserIDGuildID WHERE guild_id = $1")
                    .bind(guild_id.0 as i64)
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
                                            new.member.as_ref().unwrap().user.avatar_url().unwrap(),
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
                let guild_id = new.guild_id.unwrap();

                let to_ping_user_ids: Vec<UserIDGuildID> =
                    sqlx::query_as("SELECT * FROM UserIDGuildID WHERE guild_id = $1")
                        .bind(guild_id.0 as i64)
                        .fetch_all(&ctx.data.read().await.get::<State>().unwrap().pool)
                        .await
                        .unwrap();

                for user_id_guild_id in to_ping_user_ids {
                    let user_id = user_id_guild_id.user_id;
                    let user = match ctx.cache.user(user_id as u64) {
                        Some(user) => user,
                        None => {
                            // get user from api
                            ctx.http.get_user(user_id as u64).await.unwrap()
                        }
                    };
                    let channel_name = channel_id.name(&ctx.cache).await.unwrap();
                    if let Err(e) = user
                        .direct_message(&ctx.http, |m| {
                            m.add_embed(|e| {
                                e.title(new.guild_id.unwrap().name(&ctx.cache).unwrap())
                                    .url("https://discord.gg/invite")
                                    .author(|a| {
                                        a.name(new.member.as_ref().unwrap().display_name())
                                            .url("https://discord.gg/invite")
                                            .icon_url(
                                                new.member
                                                    .as_ref()
                                                    .unwrap()
                                                    .user
                                                    .avatar_url()
                                                    .unwrap(),
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
                command.name("vcping").description(
                    "Get added to the list of UserIDGuildID to ping when someone starts a VC",
                )
            })
        })
        .await
        .unwrap();

        info!("Slash commands registered: {:?}", commands);
    }
}

struct State {
    pool: SqlitePool,
}

impl TypeMapKey for State {
    type Value = State;
}

const DB_URL: &str = "sqlite://sqlite.db";

#[tokio::main]
async fn main() {
    let filter = tracing_subscriber::filter::Targets::new()
        .with_default(Level::DEBUG)
        .with_target("sqlx", Level::WARN);

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::Layer::default());

    // make a sqlx sqlite pool
    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        warn!("Creating database {}", DB_URL);
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => info!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    } else {
        info!("Database already exists");
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(DB_URL)
        .await
        .expect("Error connecting to database");

    pool.execute(include_str!("../schema.sql"))
        .await
        .expect("Error creating schema");

    // load the discord token from the token file
    let token = std::fs::read_to_string("token").expect("Error reading token");

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILDS
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES;

    let state = State { pool };
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
