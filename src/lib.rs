use anyhow::anyhow;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use shuttle_secrets::SecretStore;
use sqlx::{Executor, PgPool};
use tracing::{error, info};

struct Bot;

#[derive(sqlx::FromRow)]
struct UserIDGuildID {
    user_id: i64,
    guild_id: i64,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!hello" {
            if let Err(e) = msg.channel_id.say(&ctx.http, "world!").await {
                error!("Error sending message: {:?}", e);
            }
        } else if msg.content == "!vcping" {
            let guild_id = msg.guild_id.unwrap();
            // let guild = guild_id.to_partial_guild(&ctx.http).await.unwrap();
            let user_id = msg.author.id;
            let user_id_exists: Option<UserIDGuildID> =
                sqlx::query_as("SELECT * FROM users WHERE user_id = $1 AND guild_id = $2")
                    .bind(user_id.0 as i64)
                    .bind(guild_id.0 as i64)
                    .fetch_optional(&ctx.data.read().await.get::<State>().unwrap().pool)
                    .await
                    .unwrap();

            // error!("user_id_guild_id_map: {:?}", user_id_guild_id_map);

            // add user to map, if they are not already in it
            // remove user from map, if they are already in it
            if user_id_exists.is_none() {
                sqlx::query("INSERT INTO users (user_id, guild_id) VALUES ($1, $2)")
                    .bind(user_id.0 as i64)
                    .bind(guild_id.0 as i64)
                    .execute(&ctx.data.read().await.get::<State>().unwrap().pool)
                    .await
                    .unwrap();
                // send message to user
                if let Err(e) = msg
                    .channel_id
                    .say(&ctx.http, "You have been added to the ping list!")
                    .await
                {
                    error!("Error sending message: {:?}", e);
                }
            } else {
                sqlx::query("DELETE FROM users WHERE user_id = $1 AND guild_id = $2")
                    .bind(user_id.0 as i64)
                    .bind(guild_id.0 as i64)
                    .execute(&ctx.data.read().await.get::<State>().unwrap().pool)
                    .await
                    .unwrap();
                // send message to user
                if let Err(e) = msg
                    .channel_id
                    .say(&ctx.http, "You have been removed from the ping list!")
                    .await
                {
                    error!("Error sending message: {:?}", e);
                }
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, vs: VoiceState) {
        // Do nothing
        let guild_id = vs.guild_id.unwrap();

        let user_ids: Vec<UserIDGuildID> =
            sqlx::query_as("SELECT * FROM users WHERE guild_id = $1")
                .bind(guild_id.0 as i64)
                .fetch_all(&ctx.data.read().await.get::<State>().unwrap().pool)
                .await
                .unwrap();
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

struct State {
    pool: PgPool,
}

impl TypeMapKey for State {
    type Value = State;
}

#[shuttle_service::main]
async fn serenity(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> shuttle_service::ShuttleSerenity {
    pool.execute(include_str!("../schema.sql"))
        .await
        .expect("Error creating schema");

    // Get the discord token set in `Secrets.toml`
    let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
        token
    } else {
        return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
    };

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES;

    let state = State { pool };

    let client = Client::builder(&token, intents)
        .event_handler(Bot)
        .type_map_insert::<State>(state)
        .await
        .expect("Err creating client");

    Ok(client)
}
