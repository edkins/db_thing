use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
pub struct DbThingConf {
    pub google: GoogleConf,
    pub postgres: PostgresConf,
}

#[derive(Deserialize)]
pub struct GoogleConf {
    pub client_secret: String,
}

#[derive(Deserialize)]
pub struct PostgresConf {
    pub host: String,
    pub user: String,
    pub password: String,
}

impl DbThingConf {
    pub fn load() -> std::io::Result<Self> {
        let conf = fs::read_to_string("dbthing.conf")?;
        Ok(serde_json::from_str(&conf)?)
    }
}
