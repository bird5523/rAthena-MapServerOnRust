use anyhow::Result;
use sqlx::mysql::MySqlPoolOptions;
use tokio::net::TcpListener;
use tracing::{error, info, Level};

mod core;
mod database;
mod map;
mod script;
mod network;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    info!("Starting rust-map-server (Phase 1)");

    // 2. Database connection
    let db_url = dotenvy::var("DATABASE_URL").unwrap_or_else(|_| {
        "mysql://mcumaxco_ro:R2374182o@abtechth.in:3306/mcumaxco_ro".to_string()
    });

    info!("Connecting to database...");
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await;

    let pool_clone = pool.unwrap(); // Simplified for setup
    
    // 3. Load Server Configuration
    let config_path = "../rathena-master/conf/map_athena.conf";
    let config = core::config::ServerConfig::load(config_path);

    // Load Map Cache
    let map_cache_path = r"D:\AiDirectory\Ragnarok\server\rathena-master\db\map_cache.dat";
    let maps = map::map_cache::load_map_cache(map_cache_path).expect("Failed to load map cache");

    // Initialize Database Manager and load caches
    let db_manager = database::DatabaseManager::new(pool_clone).await;
    
    // Initialize Global State
    let server_state = core::state::ServerState::new(std::sync::Arc::new(db_manager), config.clone(), maps);

    // 4. Background Tasks: Auto-save loop
    let autosave_state = server_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            tracing::debug!("Running auto-save routine...");
            // TODO: Iterate over active players and save their state to DB
        }
    });

    // Initialize Map Manager (Actor system)
    let mut map_manager = map::manager::MapManager::new(server_state.clone());
    map_manager.start_map("prontera");
    map_manager.start_map("payon");
    map_manager.start_map("new_1-1"); // Explicitly adding the test map

    // Spawn Test NPC (Healer)
    use crate::script::{NpcScript, ScriptCommand};
    
    let mut script_cmds = Vec::new();
    script_cmds.push(ScriptCommand::Mes("Hello there, traveler!".to_string()));
    script_cmds.push(ScriptCommand::Next);
    script_cmds.push(ScriptCommand::Mes("I will restore your HP and SP.".to_string()));
    script_cmds.push(ScriptCommand::Heal(10000, 10000));
    script_cmds.push(ScriptCommand::Next);
    script_cmds.push(ScriptCommand::Mes("Good luck on your adventure!".to_string()));
    script_cmds.push(ScriptCommand::Close);

    let healer_script = NpcScript {
        name: "Healer".to_string(),
        sprite_id: 101, // 101: 1_f_maria
        commands: script_cmds,
    };

    if let Some(tx) = map_manager.maps.get("new_1-1") {
        let _ = tx.send(map::manager::MapMessage::SpawnNpc {
            npc_id: 20000,
            x: 53,
            y: 111,
            script: healer_script,
        }).await;

        // Spawn Warper NPC
        let mut warper_cmds = Vec::new();
        warper_cmds.push(ScriptCommand::Mes("I can teleport you to other cities.".to_string()));
        warper_cmds.push(ScriptCommand::Next);
        warper_cmds.push(ScriptCommand::Menu(vec![
            ("Prontera".to_string(), 5), 
            ("Payon".to_string(), 7),
            ("Cancel".to_string(), 9)
        ])); // Jump to indices
        warper_cmds.push(ScriptCommand::Jump(9)); // Should never hit
        warper_cmds.push(ScriptCommand::Jump(9)); // Should never hit
        warper_cmds.push(ScriptCommand::Warp("prontera".to_string(), 150, 150)); // Index 5
        warper_cmds.push(ScriptCommand::Jump(9)); // Just to space out for safety
        warper_cmds.push(ScriptCommand::Warp("payon".to_string(), 150, 150)); // Index 7
        warper_cmds.push(ScriptCommand::Jump(9));
        warper_cmds.push(ScriptCommand::Close); // Index 9
        
        let warper_script = NpcScript {
            name: "Warper".to_string(),
            sprite_id: 110,
            commands: warper_cmds,
        };

        let _ = tx.send(map::manager::MapMessage::SpawnNpc {
            npc_id: 20001,
            x: 55,
            y: 111,
            script: warper_script,
        }).await;
    }

    // Save senders to Global State
    {
        let mut senders = server_state.map_senders.write().await;
        for (name, tx) in &map_manager.maps {
            senders.insert(name.clone(), tx.clone());
        }
    }

    // Connect to Char Server in the background
    let state_for_char = server_state.clone();
    tokio::spawn(async move {
        network::inter_server::connect_to_char_server(state_for_char).await;
    });

    // 3. Start TCP listener for the map server (default port 5121)
    let port = server_state.config.map_port;
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Map Server listening on port {}", port);

    // Setup graceful shutdown signal
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c");
        info!("Received shutdown signal!");
        let _ = shutdown_tx.send(()).await;
    });

    loop {
        tokio::select! {
            Ok((socket, addr)) = listener.accept() => {
                info!("New connection from: {}", addr);
                
                let state_clone = server_state.clone();
                tokio::spawn(async move {
                    if let Err(e) = network::handle_connection(socket, state_clone).await {
                        error!("Connection error from {}: {:?}", addr, e);
                    }
                });
            }
            _ = shutdown_rx.recv() => {
                info!("Shutting down server connections...");
                map_manager.shutdown_all().await;
                break;
            }
        }
    }

    info!("Server shutdown complete.");
    Ok(())
}
