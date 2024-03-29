use serenity::{client::Context, model::channel::Message};

use crate::{config::Config, State};
use rand::prelude::SliceRandom;

pub struct JokeConfig {
    pub chance: f64,
    pub guild_id: i64,
}

pub async fn handle_jokes_message(ctx: &Context, msg: &Message) {
    // check db for chance of making a joke
    let gid = msg.guild_id.unwrap().get() as i64;
    let chance = sqlx::query_as!(
        JokeConfig,
        "SELECT * FROM JokeConfig WHERE guild_id = $1",
        gid
    )
    .fetch_optional(&ctx.data.read().await.get::<State>().unwrap().pool)
    .await
    .unwrap()
    .unwrap_or(JokeConfig {
        chance: 1.1,
        guild_id: gid,
    });

    if rand::random::<f64>() > chance.chance {
        return;
    }

    let jokes = ctx.data.read().await.get::<Config>().unwrap().jokes.clone();

    for joke in jokes {
        if let Some(servers) = joke.servers {
            if !servers.contains(&msg.guild_id.unwrap()) {
                continue;
            }
        }
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
