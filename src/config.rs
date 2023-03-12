//use regex::Regex;
use fancy_regex::Regex;

use serde::{Deserialize, Deserializer};
use serenity::prelude::TypeMapKey;
use std::fs;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub invite: String,
    pub jokes: Vec<Joke>,
}

impl TypeMapKey for Config {
    type Value = Config;
}

fn regex_from_str<'de, D>(deserializer: D) -> Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let re: String = Deserialize::deserialize(deserializer).expect("could not parse regex");
    Ok(fancy_regex::Regex::new(&re).unwrap())
}

#[derive(Deserialize, Clone, Debug)]
pub struct Joke {
    pub name: String,
    #[serde(deserialize_with = "regex_from_str")]
    pub regex: Regex,
    pub message: Vec<String>,
}

pub fn load_config() -> Config {
    let file_path = "config.json".to_owned();
    let contents = fs::read_to_string(file_path).expect("Couldn't find or load that file.");
    let p: Config = serde_json::from_str(&contents).expect("Failed to parse json");
    p
}
