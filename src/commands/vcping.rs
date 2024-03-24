use serenity::{
    all::{CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage},
    client::Context,
};

use crate::{State, UserIDGuildID};

pub async fn handle_vcping_command(ctx: &Context, command: &CommandInteraction) {
    let user_id = command.member.as_ref().unwrap().user.id.get() as i64;
    // get command options
    let disconnect_message = command
        .data
        .options
        .iter()
        .find(|option| option.name == "disconnect-message")
        .and_then(|option| option.clone().value.as_bool());
    let guild_id = command.guild_id.unwrap().get() as i64;
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
    let message_text = if user_id_exists.is_none() {
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
    };

    command
        .create_response(
            &ctx,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(message_text),
            ),
        )
        .await
        .unwrap();
}
