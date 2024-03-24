use crate::application::interaction::Interaction;
use lukas_bot::*;
use serenity::model::application::command::{Command, CommandOptionType};
use serenity::model::Permissions;
use serenity::model::{channel::Message, gateway::Ready, voice::VoiceState};
use serenity::prelude::*;
use serenity::{async_trait, model::application};
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Sqlite;
use std::collections::HashSet;
use tracing::{info, warn, Level};
use tracing_subscriber::prelude::*;

struct Bot;

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
            handle_owner_message(&ctx, &msg).await;
        }

        handle_jokes_message(&ctx, &msg).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            match command.data.name.as_str() {
                "vcping" => {
                    handle_vcping_command(&ctx, &command).await;
                }
                "joke-config" => {
                    handle_joke_config_command(&ctx, &command).await;
                }

                command => unreachable!("Unknown command: {}", command),
            };
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        handle_voice_state_update(ctx, old, new).await;
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        // register commands
        let commands = Command::set_global_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|command| {
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
                .create_application_command(|command| {
                    command
                        .name("joke-config")
                        .description("Configure how the bot should make jokes")
                        .default_member_permissions(Permissions::ADMINISTRATOR)
                        .create_option(|option| {
                            option
                                .name("chance")
                                .description("The chance that the bot will make a joke")
                                .kind(CommandOptionType::Integer)
                                .required(false)
                                .min_int_value(0)
                                .max_int_value(100)
                        })
                })
        })
        .await
        .unwrap();

        info!("Slash commands registered: {:?}", commands);
    }
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

    info!("Starting bot version {}", env!("CARGO_PKG_VERSION"));

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
