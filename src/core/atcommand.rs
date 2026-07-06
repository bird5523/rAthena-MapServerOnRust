use tokio::sync::mpsc::UnboundedSender;
use crate::network::codec::RoPacket;
use crate::map::manager::MapMessage;
use std::collections::HashMap;

pub struct AtCommandContext {
    pub char_id: u32,
    pub map_name: String,
    pub args: Vec<String>,
    pub reply_tx: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket>,
    pub map_tx: Option<tokio::sync::mpsc::Sender<crate::map::manager::MapMessage>>,
    pub packet_tx: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket>,
}

type CommandHandler = fn(AtCommandContext) -> Result<String, String>;

pub struct AtCommandRegistry {
    commands: HashMap<String, CommandHandler>,
}

impl AtCommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
        };
        registry.register("spawn", handle_spawn);
        registry.register("warp", handle_warp);
        registry.register("go", handle_go);
        registry
    }

    pub fn register(&mut self, name: &str, handler: CommandHandler) {
        self.commands.insert(name.to_lowercase(), handler);
    }

    pub async fn execute(&self, command_str: &str, mut ctx: AtCommandContext) -> Result<String, String> {
        let mut parts = command_str.trim().split_whitespace();
        let cmd_name = parts.next().unwrap_or("").trim_start_matches('@').to_lowercase();
        
        ctx.args = parts.map(|s| s.to_string()).collect();

        if let Some(handler) = self.commands.get(&cmd_name) {
            handler(ctx)
        } else {
            Err(format!("Unknown command: @{}", cmd_name))
        }
    }
}

fn handle_spawn(ctx: AtCommandContext) -> Result<String, String> {
    if ctx.args.is_empty() {
        return Err("Usage: @spawn <mob_id> [amount]".to_string());
    }

    let mob_id: u32 = ctx.args[0].parse().map_err(|_| "Invalid mob ID".to_string())?;
    let amount: u16 = if ctx.args.len() > 1 {
        ctx.args[1].parse().unwrap_or(1)
    } else {
        1
    };

    if let Some(tx) = ctx.map_tx {
        let char_id = ctx.char_id;
        let respond_to = ctx.packet_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(crate::map::manager::MapMessage::CommandSpawnMob { char_id, mob_id, amount, respond_to }).await;
        });
        Ok(format!("Spawned {} monster(s) with ID {}", amount, mob_id))
    } else {
        Err("Internal error: Map context missing".to_string())
    }
}

fn handle_warp(ctx: AtCommandContext) -> Result<String, String> {
    if ctx.args.is_empty() {
        return Err("Usage: @warp <map_name> [x] [y]".to_string());
    }

    let target_map = ctx.args[0].clone();
    let x: u16 = if ctx.args.len() > 1 { ctx.args[1].parse().unwrap_or(0) } else { 0 };
    let y: u16 = if ctx.args.len() > 2 { ctx.args[2].parse().unwrap_or(0) } else { 0 };

    if let Some(tx) = ctx.map_tx {
        let char_id = ctx.char_id;
        let reply_tx = ctx.reply_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(MapMessage::CommandWarp { char_id, target_map, x, y, respond_to: reply_tx }).await;
        });
        Ok("Warping...".to_string())
    } else {
        Err("Internal error: Map context missing".to_string())
    }
}

fn handle_go(ctx: AtCommandContext) -> Result<String, String> {
    if ctx.args.is_empty() {
        return Err("Usage: @go <id>".to_string());
    }
    
    // For now, map simple IDs to maps
    let id: u32 = ctx.args[0].parse().map_err(|_| "Invalid location ID".to_string())?;
    
    let (target_map, x, y) = match id {
        0 => ("prt_in", 63, 60), // Prontera indoor dummy
        1 => ("morocc", 156, 93),
        2 => ("geffen", 119, 59),
        3 => ("payon", 152, 233),
        4 => ("alberta", 116, 57),
        _ => return Err(format!("Unknown go location: {}", id)),
    };
    
    if let Some(tx) = ctx.map_tx {
        let char_id = ctx.char_id;
        let reply_tx = ctx.reply_tx.clone();
        let map_string = target_map.to_string();
        tokio::spawn(async move {
            let _ = tx.send(MapMessage::CommandWarp { char_id, target_map: map_string, x, y, respond_to: reply_tx }).await;
        });
        Ok(format!("Warping to {}...", target_map))
    } else {
        Err("Internal error: Map context missing".to_string())
    }
}
