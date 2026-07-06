use std::fs;
use std::path::Path;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub userid: String,
    pub passwd: String,
    pub char_ip: String,
    pub char_port: u16,
    pub map_ip: String,
    pub map_port: u16,
    pub max_connections: u32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            userid: "s1".to_string(),
            passwd: "p1".to_string(),
            char_ip: "127.0.0.1".to_string(),
            char_port: 6121,
            map_ip: "0.0.0.0".to_string(),
            map_port: 5121,
            max_connections: 5000,
        }
    }
}

impl ServerConfig {
    pub fn load(path: &str) -> Self {
        let mut config = ServerConfig::default();
        let config_path = Path::new(path);

        if !config_path.exists() {
            warn!("Config file {} not found, using default settings.", path);
            return config;
        }

        info!("Loading configuration from {}...", path);

        if let Ok(content) = fs::read_to_string(config_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                // Ignore comments and empty lines
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    continue;
                }

                if let Some((key, value)) = trimmed.split_once(':') {
                    let k = key.trim();
                    let v = value.split("//").next().unwrap_or("").trim(); // Remove inline comments

                    match k {
                        "userid" => config.userid = v.to_string(),
                        "passwd" => config.passwd = v.to_string(),
                        "char_ip" => config.char_ip = v.to_string(),
                        "char_port" => {
                            if let Ok(port) = v.parse::<u16>() {
                                config.char_port = port;
                            }
                        }
                        "map_ip" | "bind_ip" => config.map_ip = v.to_string(),
                        "map_port" => {
                            if let Ok(port) = v.parse::<u16>() {
                                config.map_port = port;
                            }
                        }
                        "max_connect_user" => {
                            if let Ok(max) = v.parse::<u32>() {
                                config.max_connections = max;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        config
    }
}
