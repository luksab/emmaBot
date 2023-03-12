-- DROP TABLE IF EXISTS UserIDGuildID;

CREATE TABLE IF NOT EXISTS UserIDGuildID (
  user_id BIGINT NOT NULL,
  guild_id BIGINT NOT NULL,
  primary key (user_id, guild_id)
);
