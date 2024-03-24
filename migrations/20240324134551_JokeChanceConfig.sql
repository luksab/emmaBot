-- Add migration script here

CREATE TABLE IF NOT EXISTS JokeConfig (
    guild_id BIGINT NOT NULL PRIMARY KEY,
    chance FLOAT NOT NULL
);
