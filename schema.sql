DROP TABLE IF EXISTS users;

CREATE TABLE users (
  user_id BIGINT NOT NULL,
  guild_id BIGINT NOT NULL,
  primary key (user_id, guild_id)
);
