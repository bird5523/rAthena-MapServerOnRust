use std::sync::Arc;
use crate::database::DatabaseManager;
use crate::core::config::ServerConfig;
use crate::map::map_instance::MapInstance;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Party {
    pub id: u32,
    pub name: String,
    pub leader_id: u32,
    pub members: Vec<u32>, // char_ids
}

#[derive(Debug, Clone)]
pub struct Guild {
    pub id: u32,
    pub name: String,
    pub leader_id: u32,
    pub level: u16,
    pub members: Vec<u32>, // char_ids
}

#[derive(bevy_ecs::prelude::Resource, Clone)]
pub struct GlobalState(pub Arc<ServerState>);

/// Global State shared across the entire server
pub struct ServerState {
    pub db_manager: Arc<DatabaseManager>,
    pub config: ServerConfig,
    pub maps: Arc<HashMap<String, MapInstance>>,
    pub map_senders: Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::mpsc::Sender<crate::map::manager::MapMessage>>>>,
    pub parties: Arc<tokio::sync::RwLock<HashMap<u32, Party>>>,
    pub guilds: Arc<tokio::sync::RwLock<HashMap<u32, Guild>>>,
}

impl ServerState {
    pub fn new(db_manager: Arc<DatabaseManager>, config: ServerConfig, maps: HashMap<String, MapInstance>) -> Arc<Self> {
        Arc::new(Self {
            db_manager,
            config,
            maps: Arc::new(maps),
            map_senders: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            parties: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            guilds: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_party_management() {
        let parties = Arc::new(tokio::sync::RwLock::new(HashMap::<u32, Party>::new()));
        
        let p = Party {
            id: 1,
            name: "TestParty".to_string(),
            leader_id: 150000,
            members: vec![150000, 150001],
        };
        
        // Write test
        {
            let mut write_guard = parties.write().await;
            write_guard.insert(p.id, p);
        }
        
        // Read test
        {
            let read_guard = parties.read().await;
            assert!(read_guard.contains_key(&1));
            assert_eq!(read_guard.get(&1).unwrap().name, "TestParty");
            assert_eq!(read_guard.get(&1).unwrap().members.len(), 2);
        }
    }
}
