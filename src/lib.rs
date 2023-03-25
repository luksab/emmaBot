use serenity::client::Context;
use serenity::model::voice::VoiceState;

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
