use std::path::Path;

use anyhow::Result;
use firefly_server::config::{self, ServerConfig};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CardanoConnectConfig {
    pub api: ServerConfig,
}

pub fn load_config(config_file: Option<&Path>) -> Result<CardanoConnectConfig> {
    config::load_config(
        "cardanoconnect",
        include_str!("../config.base.yaml"),
        config_file,
    )
}
