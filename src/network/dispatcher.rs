use tracing::{info, warn, error};
use std::sync::Arc;
use crate::core::state::ServerState;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use super::codec::{RoPacketCodec, RoPacket};
use futures_util::SinkExt;

fn encode_pos2(sx: u16, sy: u16, dx: u16, dy: u16, sdir: u8, ddir: u8) -> [u8; 6] {
    let mut buf = [0u8; 6];
    buf[0] = (sx >> 2) as u8;
    buf[1] = (((sx << 6) & 0xC0) as u8) | (((sy >> 4) & 0x3F) as u8);
    buf[2] = (((sy << 4) & 0xF0) as u8) | (((dx >> 6) & 0x0F) as u8);
    buf[3] = (((dx << 2) & 0xFC) as u8) | (((dy >> 8) & 0x03) as u8);
    buf[4] = (dy & 0xFF) as u8;
    buf[5] = (sdir << 4) | (ddir & 0x0F);
    buf
}

pub struct Dispatcher;

impl Dispatcher {
    pub async fn dispatch(packet: RoPacket, packet_tx: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket>, _state: Arc<ServerState>) -> anyhow::Result<()> {
        let cmd = packet.cmd;
        
        match cmd {
            0x0436 => {
                info!("Client requested Map Login (0x0436)!");
                // Client wants to enter the map. We must respond with ZC_ACCEPT_ENTER.
                // Depending on the client version, it might be 0x0073, 0x02eb, or 0x0a18.
                // We will use 0x02eb which is standard for mid-to-new clients.
                
                let mut response = vec![0u8; 13];
                response[0] = 0xeb;
                response[1] = 0x02; // 0x02eb
                
                // start time (dummy 0 for now)
                response[2..6].copy_from_slice(&0u32.to_le_bytes());
                
                // posDir (3 bytes)
                // Let's spawn them at new_1-1 53, 111, dir 4
                let x: u16 = 53;
                let y: u16 = 111;
                let dir: u8 = 4;
                
                response[6] = (x >> 2) as u8;
                response[7] = (((x << 6) & 0xC0) | ((y >> 4) & 0x3F)) as u8;
                response[8] = (((y << 4) & 0xF0) | ((dir as u16) & 0xF)) as u8;
                
                // xSize, ySize
                response[9] = 5;
                response[10] = 5;
                
                // font
                response[11] = 0;
                response[12] = 0;
                
                let out_packet = RoPacket {
                    cmd: 0x02eb,
                    payload: response[2..].to_vec(),
                };
                
                if let Err(e) = packet_tx.send(out_packet) {
                    error!("Failed to send ZC_ACCEPT_ENTER (0x02eb) to client: {}", e);
                } else {
                    info!("Sent ZC_ACCEPT_ENTER (0x02eb). Waiting for client to load map...");
                }
            }
            0x007d => {
                info!("Client sent LoadEndAck (0x007d)! They have loaded the map!");
                
                // 1. Send ZC_NOTIFY_MAPPROPERTY (0x0199)
                let mut map_prop = vec![0u8; 4];
                map_prop[0] = 0x99;
                map_prop[1] = 0x01;
                map_prop[2] = 0; // type (0 = nothing/normal)
                map_prop[3] = 0;
                
                if let Err(e) = packet_tx.send(RoPacket { cmd: 0x0199, payload: map_prop[2..].to_vec() }) {
                    error!("Failed to send Map Property: {}", e);
                }
                
                // 2. Send ZC_NOTIFY_TIME (0x007f)
                let mut time_pkt = vec![0u8; 6];
                time_pkt[0] = 0x7f;
                time_pkt[1] = 0x00;
                let current_time = 100000u32; // Dummy time
                time_pkt[2..6].copy_from_slice(&current_time.to_le_bytes());
                
                if let Err(e) = packet_tx.send(RoPacket { cmd: 0x007f, payload: time_pkt[2..].to_vec() }) {
                    error!("Failed to send Time: {}", e);
                }
                
                info!("Sent Map Property and Time packets. Client should now stay connected.");
                
                // SPAWN THE PLAYER IN THE MAP MANAGER!
                let senders = _state.map_senders.read().await;
                let map_name = "new_1-1"; // TODO: get from session
                let char_id = 150000; // TODO: get from session
                if let Some(tx) = senders.get(map_name) {
                    let (reply_tx, _reply_rx) = tokio::sync::oneshot::channel();
                    let _ = tx.send(crate::map::manager::MapMessage::PlayerEnter {
                        char_id,
                        packet_tx: packet_tx.clone(),
                        respond_to: reply_tx,
                    }).await;
                }
                
                // TODO: Implement dynamic PACKETVER serialization for clif_spawn_unit (ZC_NOTIFY_STANDENTRY)
            }
            0x035f => {
                // CZ_REQUEST_MOVE: <cmd: u16> <dest_xy_dir: u8[3]>
                if packet.payload.len() >= 3 {
                    let dest_x = ((packet.payload[0] as u16) << 2) | ((packet.payload[1] as u16) >> 6);
                    let dest_y = (((packet.payload[1] as u16) & 0x3F) << 4) | ((packet.payload[2] as u16) >> 4);
                    let _dir = packet.payload[2] & 0x0F;

                    info!("Client requests move to {},{}", dest_x, dest_y);
                    
                    let map_name = "new_1-1"; // TODO: get from session
                    let char_id = 150000; // TODO: get from session

                    let senders = _state.map_senders.read().await;
                    let mut actual_sx = 53;
                    let mut actual_sy = 111;
                    let mut is_new = true;
                    let mut delay_ms = 0;
                    
                    if let Some(tx) = senders.get(map_name) {
                        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                        if tx.send(crate::map::manager::MapMessage::PlayerMove { char_id, x: dest_x, y: dest_y, respond_to: reply_tx }).await.is_ok() {
                            if let Ok(Some((sx, sy, delay, new_target))) = reply_rx.await {
                                actual_sx = sx;
                                actual_sy = sy;
                                delay_ms = delay;
                                is_new = new_target;
                            }
                        }
                    }

                    if is_new {
                        let packet_tx_clone = packet_tx.clone();
                        tokio::spawn(async move {
                            if delay_ms > 0 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                            }
                            
                            // Send ZC_NOTIFY_PLAYERMOVE (0x0087) back to the client so they can walk
                            let mut ack = vec![0u8; 12];
                            ack[0] = 0x87;
                            ack[1] = 0x00; // 0x0087
                            
                            let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u32;
                            ack[2..6].copy_from_slice(&tick.to_le_bytes());
                            
                            let pos_data = encode_pos2(actual_sx, actual_sy, dest_x, dest_y, 8, 8);
                            ack[6..12].copy_from_slice(&pos_data);
                            
                            if let Err(e) = packet_tx_clone.send(RoPacket { cmd: 0x0087, payload: ack[2..].to_vec() }) {
                                error!("Failed to send WalkOk: {}", e);
                            }
                        });
                    }
                }
            }
            0x0089 | 0x0360 | 0x0437 => {
                // CZ_REQUEST_ACT: <cmd: u16> <target_id: u32> <action: u8>
                if packet.payload.len() >= 4 {
                    let target_id = u32::from_le_bytes(packet.payload[0..4].try_into().unwrap());
                    let action = if packet.payload.len() >= 5 { packet.payload[4] } else { 0 };

                    info!("Client requested action {} on target {}", action, target_id);
                    
                    if action == 0 {
                        let map_name = "new_1-1"; // TODO: get from session
                        let char_id = 150000; // TODO: get from session

                        let senders = _state.map_senders.read().await;
                        if let Some(tx) = senders.get(map_name) {
                            let _ = tx.send(crate::map::manager::MapMessage::PlayerInteract { char_id, target_id, respond_to: packet_tx.clone() }).await;
                        }
                    } else if action == 7 { // 7 is usually pickup in newer clients
                        let map_name = "new_1-1"; // TODO: get from session
                        let char_id = 150000; // TODO: get from session

                        let senders = _state.map_senders.read().await;
                        if let Some(tx) = senders.get(map_name) {
                            let _ = tx.send(crate::map::manager::MapMessage::PickupItem { char_id, ground_entity_id: target_id }).await;
                        }
                    }
                }
            }
            0x0085 | 0x008c | 0x00f3 | 0x009f => {
                // CZ_REQUEST_CHAT: <cmd: u16> <packet_len: u16> <message...>
                info!("[CHAT DEBUG] Received chat packet (cmd: {:#06x}) with payload size: {}", packet.cmd, packet.payload.len());
                let msg_bytes = &packet.payload;
                info!("[CHAT DEBUG] Raw payload bytes: {:?}", msg_bytes);
                
                let mut msg = String::from_utf8_lossy(msg_bytes).into_owned();
                let has_null = msg.ends_with('\0');
                if has_null {
                    msg.pop();
                }
                info!("[CHAT DEBUG] Parsed string (null-terminated: {}): {}", has_null, msg);
                
                let map_name = "new_1-1"; // TODO: get from session
                let char_id = 150000; // TODO: get from session
                
                // Check for AT Command
                if let Some(cmd_part) = msg.split(" : ").nth(1) {
                    if cmd_part.starts_with('@') {
                        let mut registry = crate::core::atcommand::AtCommandRegistry::new();
                        let senders = _state.map_senders.read().await;
                        let map_tx = senders.get(map_name).cloned();
                        
                        let ctx = crate::core::atcommand::AtCommandContext {
                            char_id,
                            map_name: map_name.to_string(),
                            args: Vec::new(),
                            reply_tx: packet_tx.clone(),
                            packet_tx: packet_tx.clone(),
                            map_tx,
                        };
                        
                        match registry.execute(cmd_part, ctx).await {
                            Ok(res) | Err(res) => {
                                // Send response as a server message (Yellow text usually, but for now we'll just whisper it to self)
                                // ZC_WHISPER (0x0097)
                                let mut reply = vec![0u8; 28 + res.len() + 1];
                                reply[0] = 0x97;
                                reply[1] = 0x00;
                                let len = reply.len() as u16;
                                reply[2..4].copy_from_slice(&len.to_le_bytes());
                                let sender_name = b"Server\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
                                reply[4..28].copy_from_slice(&sender_name[..24]);
                                reply[28..28 + res.len()].copy_from_slice(res.as_bytes());
                                
                                let _ = packet_tx.send(RoPacket { cmd: 0x0097, payload: reply[2..].to_vec() });
                            }
                        }
                        
                        // Still send 0x008e to clear the input box
                        let mut reply = vec![0u8; 4 + msg_bytes.len()];
                        reply[0] = 0x8e;
                        reply[1] = 0x00;
                        let len = reply.len() as u16;
                        reply[2..4].copy_from_slice(&len.to_le_bytes());
                        reply[4..].copy_from_slice(msg_bytes);
                        let _ = packet_tx.send(RoPacket { cmd: 0x008e, payload: reply[2..].to_vec() });
                        
                        return Ok(());
                    }
                }
                
                // Normal chat message
                // 1. Send ZC_ACK_REQCHAT (0x008e) to clear the input box
                let mut reply = vec![0u8; 4 + msg_bytes.len()];
                reply[0] = 0x8e;
                reply[1] = 0x00;
                let len = reply.len() as u16;
                reply[2..4].copy_from_slice(&len.to_le_bytes());
                reply[4..].copy_from_slice(msg_bytes);
                
                info!("[CHAT DEBUG] Sending ZC_ACK_REQCHAT (0x008e). Reply size: {}. Raw bytes: {:?}", reply.len(), reply);
                if let Err(e) = packet_tx.send(RoPacket { cmd: 0x008e, payload: reply[2..].to_vec() }) {
                    error!("Failed to send chat reply: {}", e);
                }
                
                // 2. Send ZC_NOTIFY_CHAT (0x008d) so the player can SEE their own message
                // (Without this, the client's chat box might feel "stuck" or the text won't appear)
                let mut notify = vec![0u8; 8 + msg_bytes.len()];
                notify[0] = 0x8d;
                notify[1] = 0x00;
                let len = notify.len() as u16;
                notify[2..4].copy_from_slice(&len.to_le_bytes());
                notify[4..8].copy_from_slice(&char_id.to_le_bytes()); // Use char_id as GID
                notify[8..].copy_from_slice(msg_bytes);
                
                info!("[CHAT DEBUG] Sending ZC_NOTIFY_CHAT (0x008d).");
                let _ = packet_tx.send(RoPacket { cmd: 0x008d, payload: notify[2..].to_vec() });
            }
            0x0096 => {
                // CZ_WHISPER
                info!("[CHAT DEBUG] Received Whisper packet (0x0096) with payload size: {}", packet.payload.len());
                info!("[CHAT DEBUG] Raw whisper payload bytes: {:?}", packet.payload);
                
                let mut reply = vec![0u8; 5];
                reply[0] = 1; // result = 1 (not logged in)
                
                info!("[CHAT DEBUG] Sending ZC_ACK_WHISPER (0x0098). Raw payload bytes: {:?}", reply);
                
                if let Err(e) = packet_tx.send(RoPacket { cmd: 0x0098, payload: reply }) {
                    error!("Failed to send whisper reply: {}", e);
                }
            }
            0x0113 => {
                // CZ_USE_SKILL: <cmd: u16> <skill_lv: u16> <skill_id: u16> <target_id: u32>
                if packet.payload.len() >= 8 {
                    let skill_lv = u16::from_le_bytes(packet.payload[0..2].try_into().unwrap());
                    let skill_id = u16::from_le_bytes(packet.payload[2..4].try_into().unwrap());
                    let target_id = u32::from_le_bytes(packet.payload[4..8].try_into().unwrap());
                    
                    info!("Client used skill {} (Lv {}) on target {}", skill_id, skill_lv, target_id);
                    
                    let map_name = "new_1-1"; // TODO: get from session
                    let char_id = 150000; // TODO: get from session
                    let senders = _state.map_senders.read().await;
                    if let Some(tx) = senders.get(map_name) {
                        let _ = tx.send(crate::map::manager::MapMessage::UseSkill {
                            char_id,
                            skill_id,
                            skill_level: skill_lv,
                            target: crate::core::components::SkillTarget::Entity(target_id),
                        }).await;
                    }
                }
            }
            0x0090 => {
                // CZ_CONTACTNPC: <cmd: u16> <target_id: u32> <type: u8>
                if packet.payload.len() >= 5 {
                    let target_id = u32::from_le_bytes(packet.payload[0..4].try_into().unwrap());
                    let map_name = "new_1-1"; 
                    let char_id = 150000; 
                    let senders = _state.map_senders.read().await;
                    if let Some(tx) = senders.get(map_name) {
                        let (reply_tx, mut reply_rx) = tokio::sync::mpsc::unbounded_channel();
                        let _ = tx.send(crate::map::manager::MapMessage::NpcClick {
                            char_id,
                            npc_id: target_id,
                            respond_to: reply_tx,
                        }).await;

                        // Drain responses
                        while let Ok(reply_pkt) = reply_rx.try_recv() {
                            let _ = packet_tx.send(reply_pkt);
                        }
                    }
                }
            }
            0x00b9 => {
                // CZ_REQ_NEXT_SCRIPT: <cmd: u16> <target_id: u32>
                if packet.payload.len() >= 4 {
                    let target_id = u32::from_le_bytes(packet.payload[0..4].try_into().unwrap());
                    let map_name = "new_1-1"; 
                    let char_id = 150000; 
                    let senders = _state.map_senders.read().await;
                    if let Some(tx) = senders.get(map_name) {
                        let (reply_tx, mut reply_rx) = tokio::sync::mpsc::unbounded_channel();
                        let _ = tx.send(crate::map::manager::MapMessage::NpcNext { char_id, npc_id: target_id, respond_to: reply_tx }).await;
                        while let Ok(reply_pkt) = reply_rx.try_recv() {
                            let _ = packet_tx.send(reply_pkt);
                        }
                    }
                }
            }
            0x0146 => {
                // CZ_CLOSE_DIALOG: <cmd: u16> <target_id: u32>
                if packet.payload.len() >= 4 {
                    let target_id = u32::from_le_bytes(packet.payload[0..4].try_into().unwrap());
                    let map_name = "new_1-1"; 
                    let char_id = 150000; 
                    let senders = _state.map_senders.read().await;
                    if let Some(tx) = senders.get(map_name) {
                        let (reply_tx, mut reply_rx) = tokio::sync::mpsc::unbounded_channel();
                        let _ = tx.send(crate::map::manager::MapMessage::NpcClose { char_id, npc_id: target_id, respond_to: reply_tx }).await;
                        while let Ok(reply_pkt) = reply_rx.try_recv() {
                            let _ = packet_tx.send(reply_pkt);
                        }
                    }
                }
            }
            0x00b8 => {
                // CZ_CHOOSE_MENU: <cmd: u16> <target_id: u32> <selection: u8>
                if packet.payload.len() >= 5 {
                    let target_id = u32::from_le_bytes(packet.payload[0..4].try_into().unwrap());
                    let selection = packet.payload[4];
                    let map_name = "new_1-1"; 
                    let char_id = 150000; 
                    let senders = _state.map_senders.read().await;
                    if let Some(tx) = senders.get(map_name) {
                        let (reply_tx, mut reply_rx) = tokio::sync::mpsc::unbounded_channel();
                        let _ = tx.send(crate::map::manager::MapMessage::NpcMenu { char_id, npc_id: target_id, selection, respond_to: reply_tx }).await;
                        while let Ok(reply_pkt) = reply_rx.try_recv() {
                            let _ = packet_tx.send(reply_pkt);
                        }
                    }
                }
            }
            0x00b2 => {
                // CZ_RESTART: back to character select
                info!("Client requested back to character select (0x00b2)");
                // Reply with ZC_RESTART_ACK (0x00b3) with type 1 (success)
                let reply = vec![0xb3, 0x00, 0x01]; // cmd: 0x00b3, payload: [1]
                let _ = packet_tx.send(RoPacket {
                    cmd: 0x00b3,
                    payload: reply[2..].to_vec(),
                });
            }
            0x0360 | 0x0368 | 0x0b1c | 0x08c9 | 0x021d | 0x0187 => {
                // Background ping / tick sync packets from client (can be ignored)
            }
            _ => {
                warn!("Unhandled packet ID: {:#06x}", cmd);
            }
        }
        Ok(())
    }
}
