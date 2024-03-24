use serenity::{client::Context, model::channel::Message};

use crate::{State, UserIDGuildID};

pub async fn handle_owner_message(ctx: &Context, msg: &Message) {
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
