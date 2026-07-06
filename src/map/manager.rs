use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, debug, warn};
use std::collections::HashMap;
use bevy_ecs::prelude::*;
use crate::core::systems::{movement_system, ai_system, skill_system, battle_system};
use crate::core::components::{Position, Velocity, EntityStats};

/// Message types that can be sent to a Map Actor
pub enum MapMessage {
    PlayerEnter { char_id: u32, packet_tx: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket>, respond_to: oneshot::Sender<bool> },
    PlayerLeave { char_id: u32 },
    PlayerMove { char_id: u32, x: u16, y: u16, respond_to: tokio::sync::oneshot::Sender<Option<(u16, u16, u64, bool)>> },
    PlayerInteract { char_id: u32, target_id: u32, respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket> },
    UseSkill { char_id: u32, skill_id: u16, skill_level: u16, target: crate::core::components::SkillTarget },
    NpcClick { char_id: u32, npc_id: u32, respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket> },
    NpcNext { char_id: u32, npc_id: u32, respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket> },
    NpcClose { char_id: u32, npc_id: u32, respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket> },
    NpcMenu { char_id: u32, npc_id: u32, selection: u8, respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket> },
    DropItem { char_id: u32, item_id: i32, amount: u16 },
    PickupItem { char_id: u32, ground_entity_id: u32 },
    SpawnNpc { npc_id: u32, x: u16, y: u16, script: crate::script::NpcScript },
    SpawnMob { mob_id: u32, x: u16, y: u16 },
    CommandSpawnMob { char_id: u32, mob_id: u32, amount: u16, respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket> },
    CommandWarp { char_id: u32, target_map: String, x: u16, y: u16, respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket> },
    Shutdown,
}

pub struct MapActor {
    pub name: String,
    pub receiver: mpsc::Receiver<MapMessage>,
    pub world: World,
    pub schedule: Schedule,
    pub server_state: Arc<crate::core::state::ServerState>,
}

impl MapActor {
    pub fn new(name: String, receiver: mpsc::Receiver<MapMessage>, server_state: Arc<crate::core::state::ServerState>) -> Self {
        let mut world = World::new();
        world.insert_resource(crate::core::state::GlobalState(server_state.clone()));
        
        let mut schedule = Schedule::default();

        // Add systems
        schedule.add_systems(crate::core::systems::movement_system);
        schedule.add_systems(crate::core::systems::ai_system);
        schedule.add_systems(crate::core::systems::battle_system);
        schedule.add_systems(crate::core::systems::skill_system);
        schedule.add_systems(crate::core::systems::status_effect_system);

        Self { name, receiver, world, schedule, server_state }
    }

    pub async fn run(&mut self) {
        info!("Map actor '{}' started.", self.name);
        let mut tick_interval = tokio::time::interval(tokio::time::Duration::from_millis(50)); // 20 TPS

        loop {
            tokio::select! {
                Some(msg) = self.receiver.recv() => {
                    match msg {
                        MapMessage::PlayerEnter { char_id, packet_tx, respond_to } => {
                            debug!("Player {} entered map {}", char_id, self.name);
                            
                            // Spawn ECS Entity for the Player
                            self.world.spawn((
                                crate::core::components::PlayerConnection { tx: packet_tx },
                                Position {
                                    map_name: self.name.clone(),
                                    x: 53,
                                    y: 111,
                                    dir: 4,
                                },
                                Velocity {
                                    speed: 150,
                                    target: None,
                                    pending_target: None,
                                    next_move_tick: std::time::Instant::now(),
                                },
                                EntityStats {
                                    account_id: 2000000, // Dummy
                                    char_id,
                                    name: format!("Player_{}", char_id),
                                    class: 0, // Novice
                                    base_level: 1,
                                    job_level: 1,
                                    hp: 40,
                                    max_hp: 40,
                                    sp: 11,
                                    max_sp: 11,
                                    str: 1,
                                    agi: 1,
                                    vit: 1,
                                    int: 1,
                                    dex: 1,
                                    luk: 1, atk: 10, def: 10,
                                    party_id: 0, guild_id: 0,
                                },
                                crate::core::components::Inventory::new(),
                            ));

                            let _ = respond_to.send(true);
                        }
                        MapMessage::PlayerLeave { char_id } => {
                            debug!("Player {} left map {}", char_id, self.name);
                            // TODO: Remove entity
                        }
                        MapMessage::PlayerMove { char_id, x, y, respond_to } => {
                            use crate::core::components::{EntityStats, Velocity, Position};
                            // Query ECS for this char_id and update Velocity target
                            let mut query = self.world.query::<(&EntityStats, &mut Velocity, &mut Position)>();
                            let mut current_pos = None;
                            for (stats, mut vel, mut pos) in query.iter_mut(&mut self.world) {
                                if stats.char_id == char_id {
                                    let is_new = vel.target != Some((x, y));
                                    
                                    let mut start_x = pos.x;
                                    let mut start_y = pos.y;
                                    let mut delay_ms = 0;
                                    let now = std::time::Instant::now();
                                    
                                    if vel.target.is_some() && now < vel.next_move_tick && is_new {
                                        // We are currently moving to a DIFFERENT cell.
                                        // The NEXT cell we will arrive at is our new starting point for the client WalkOk.
                                        let (tx, ty) = vel.target.unwrap();
                                        if pos.x < tx { start_x += 1; }
                                        else if pos.x > tx { start_x -= 1; }
                                        
                                        if pos.y < ty { start_y += 1; }
                                        else if pos.y > ty { start_y -= 1; }
                                        
                                        let delay_ms = vel.next_move_tick.duration_since(now).as_millis() as u64;
                                        
                                        // Queue up the new target! It will be applied when the current step completes.
                                        vel.pending_target = Some((x, y));
                                        
                                        // Delay sending the WalkOk packet until the client has reached the next cell
                                        current_pos = Some((start_x, start_y, delay_ms, true));
                                    } else if is_new {
                                        vel.target = Some((x, y));
                                        current_pos = Some((pos.x, pos.y, 0, true));
                                    } else {
                                        current_pos = Some((pos.x, pos.y, 0, false));
                                    }
                                    break;
                                }
                            }
                            let _ = respond_to.send(current_pos);
                        }
                        MapMessage::PlayerInteract { char_id, target_id, respond_to } => {
                            use crate::core::components::{EntityStats, AttackTarget, NextAttackTick, Npc, MobAi};
                            
                            let mut target_is_npc = false;
                            let mut target_is_mob = false;
                            
                            let mut query_target = self.world.query::<(&EntityStats, Option<&Npc>, Option<&MobAi>)>();
                            for (stats, npc, mob) in query_target.iter(&self.world) {
                                if stats.char_id == target_id {
                                    if npc.is_some() { target_is_npc = true; }
                                    if mob.is_some() { target_is_mob = true; }
                                    break;
                                }
                            }
                            
                            if target_is_npc {
                                Self::handle_script_action(&mut self.world, char_id, target_id, respond_to, true, false, false, None);
                            } else if target_is_mob {
                                let mut player_entity = None;
                                let mut player_pos = None;
                                
                                let mut query = self.world.query::<(Entity, &EntityStats, &Position)>();
                                for (entity, stats, pos) in query.iter(&self.world) {
                                    if stats.char_id == char_id {
                                        player_entity = Some(entity);
                                        player_pos = Some((pos.x, pos.y));
                                        break;
                                    }
                                }
                                
                                // Find mob position
                                let mut mob_pos = None;
                                let mut mob_query = self.world.query::<(&EntityStats, &Position)>();
                                for (stats, pos) in mob_query.iter(&self.world) {
                                    if stats.char_id == target_id {
                                        mob_pos = Some((pos.x, pos.y));
                                        break;
                                    }
                                }
                                
                                if let (Some(p_ent), Some((px, py)), Some((mx, my))) = (player_entity, player_pos, mob_pos) {
                                    let dist = (px as i32 - mx as i32).abs() + (py as i32 - my as i32).abs();
                                    if dist > 1 {
                                        // Too far! Walk to mob first.
                                        if let Some(mut vel) = self.world.get_mut::<Velocity>(p_ent) {
                                            vel.target = Some((mx, my));
                                        }
                                        
                                        // Send ZC_NOTIFY_PLAYERMOVE (0x0087) to client to start walking
                                        let mut ack = vec![0u8; 12];
                                        ack[0] = 0x87;
                                        ack[1] = 0x00;
                                        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u32;
                                        ack[2..6].copy_from_slice(&tick.to_le_bytes());
                                        
                                        let mut pos_p = [0u8; 6];
                                        pos_p[0] = (px >> 2) as u8;
                                        pos_p[1] = (((px << 6) & 0xC0) | ((py >> 4) & 0x3F)) as u8;
                                        pos_p[2] = (((py << 4) & 0xF0) | ((mx >> 6) & 0x0F)) as u8;
                                        pos_p[3] = (((mx << 2) & 0xFC) | ((my >> 8) & 0x03)) as u8;
                                        pos_p[4] = my as u8;
                                        pos_p[5] = 0x88;
                                        ack[6..12].copy_from_slice(&pos_p);
                                        
                                        let _ = respond_to.send(crate::network::codec::RoPacket {
                                            cmd: 0x0087,
                                            payload: ack[2..].to_vec(),
                                        });
                                    }
                                    
                                    // Target it anyway, will hit when in range
                                    self.world.entity_mut(p_ent).insert(AttackTarget(target_id));
                                    self.world.entity_mut(p_ent).insert(NextAttackTick(std::time::Instant::now()));
                                    tracing::info!("Player {} targeted mob {} (dist: {})", char_id, target_id, dist);
                                }
                            } else {
                                tracing::debug!("Player {} clicked on target {}", char_id, target_id);
                            }
                        }
                        MapMessage::UseSkill { char_id, skill_id, skill_level, target } => {
                            debug!("Player {} using skill {} (Lv {})", char_id, skill_id, skill_level);
                            
                            // Look up skill in DB
                            let skill_opt = self.server_state.db_manager.skills.skills.get(&skill_id).cloned();
                            
                            if let Some(skill_data) = skill_opt {
                                let lvl_idx = (skill_level as usize).saturating_sub(1).min(9);
                                let sp_cost = skill_data.sp_cost[lvl_idx];
                                let cast_time = skill_data.cast_time[lvl_idx];
                                let multiplier = skill_data.damage_multiplier[lvl_idx];
                                let skill_type = skill_data.skill_type;

                                let mut query = self.world.query::<(Entity, &mut crate::core::components::EntityStats)>();
                                let mut attacker_entity = None;
                                let mut has_enough_sp = false;

                                for (entity, mut stats) in query.iter_mut(&mut self.world) {
                                    if stats.char_id == char_id {
                                        if stats.sp >= sp_cost {
                                            stats.sp -= sp_cost;
                                            has_enough_sp = true;
                                            attacker_entity = Some(entity);
                                        }
                                        break;
                                    }
                                }
                                
                                if has_enough_sp {
                                    if let Some(entity) = attacker_entity {
                                        self.world.entity_mut(entity).insert(crate::core::components::SkillCasting {
                                            skill_id,
                                            skill_level,
                                            target,
                                            cast_time,
                                            start_tick: std::time::Instant::now(),
                                            skill_type,
                                            multiplier,
                                        });
                                        info!("Started casting skill {} with cast time {}ms", skill_id, cast_time);
                                    }
                                } else {
                                    warn!("Player {} does not have enough SP to cast skill {}", char_id, skill_id);
                                }
                            } else {
                                warn!("Player {} tried to use unknown skill {}", char_id, skill_id);
                            }
                        }
                        MapMessage::NpcClick { char_id, npc_id, respond_to } => {
                            Self::handle_script_action(&mut self.world, char_id, npc_id, respond_to, true, false, false, None);
                        }
                        MapMessage::NpcNext { char_id, npc_id, respond_to } => {
                            Self::handle_script_action(&mut self.world, char_id, npc_id, respond_to, false, true, false, None);
                        }
                        MapMessage::NpcClose { char_id, npc_id, respond_to } => {
                            Self::handle_script_action(&mut self.world, char_id, npc_id, respond_to, false, false, true, None);
                        }
                        MapMessage::NpcMenu { char_id, npc_id, selection, respond_to } => {
                            Self::handle_script_action(&mut self.world, char_id, npc_id, respond_to, false, false, false, Some(selection));
                        }
                        MapMessage::DropItem { char_id, item_id, amount } => {
                            use crate::core::components::{EntityStats, Position, GroundItem, Inventory};
                            let mut drop_pos = None;
                            
                            // Find player to take items from and get their position
                            let mut query = self.world.query::<(&EntityStats, &mut Inventory, &Position)>();
                            for (stats, mut inv, pos) in query.iter_mut(&mut self.world) {
                                if stats.char_id == char_id {
                                    if inv.remove_item(item_id, amount) {
                                        drop_pos = Some((pos.x, pos.y, pos.map_name.clone()));
                                    }
                                    break;
                                }
                            }
                            
                            if let Some((x, y, map_name)) = drop_pos {
                                self.world.spawn((
                                    Position { map_name, x, y, dir: 0 },
                                    GroundItem {
                                        item_id,
                                        amount,
                                        dropped_by: char_id,
                                        drop_time: std::time::Instant::now(),
                                    },
                                ));
                                debug!("Player {} dropped {}x item {}", char_id, amount, item_id);
                            }
                        }
                        MapMessage::PickupItem { char_id, ground_entity_id } => {
                            use crate::core::components::{EntityStats, GroundItem, Inventory, Position};
                            
                            let mut ground_query = self.world.query::<(bevy_ecs::entity::Entity, &GroundItem, &Position)>();
                            let mut item_to_give = None;
                            let mut entity_to_despawn = None;
                            let mut item_pos = None;

                            for (entity, g_item, pos) in ground_query.iter(&self.world) {
                                if (entity.to_bits() as u32) == ground_entity_id {
                                    item_to_give = Some((g_item.item_id, g_item.amount));
                                    entity_to_despawn = Some(entity);
                                    item_pos = Some(pos.clone());
                                    break;
                                }
                            }
                            
                            if let (Some(entity), Some((i_id, amt)), Some(i_pos)) = (entity_to_despawn, item_to_give, item_pos) {
                                let mut inv_query = self.world.query::<(&EntityStats, &mut Inventory, &Position)>();
                                let mut found = false;
                                let mut within_range = false;

                                for (stats, mut inv, p_pos) in inv_query.iter_mut(&mut self.world) {
                                    if stats.char_id == char_id {
                                        // Check distance (allow pickup within 2 cells)
                                        let dx = (p_pos.x as i32 - i_pos.x as i32).abs();
                                        let dy = (p_pos.y as i32 - i_pos.y as i32).abs();
                                        
                                        if dx <= 2 && dy <= 2 {
                                            inv.add_item(i_id, amt);
                                            found = true;
                                            within_range = true;
                                        } else {
                                            warn!("Player {} is too far to pick up item. Dist: {}, {}", char_id, dx, dy);
                                        }
                                        break;
                                    }
                                }
                                
                                if found {
                                    self.world.despawn(entity);
                                    debug!("Player {} picked up {}x item {}", char_id, amt, i_id);
                                } else if !within_range {
                                    // Could send packet back: Pickup Failed
                                }
                            }
                        }
                        MapMessage::SpawnNpc { npc_id, x, y, script } => {
                            use crate::core::components::{EntityStats, Npc};
                            self.world.spawn((
                                EntityStats {
                                    account_id: 0,
                                    char_id: npc_id,
                                    name: script.name.clone(),
                                    class: 0,
                                    base_level: 1,
                                    job_level: 1,
                                    hp: 1,
                                    max_hp: 1,
                                    sp: 1,
                                    max_sp: 1,
                                    str: 1, agi: 1, vit: 1, int: 1, dex: 1, luk: 1, atk: 10, def: 10,
                                    party_id: 0, guild_id: 0,
                                },
                                Position { map_name: self.name.clone(), x, y, dir: 4 },
                                Npc {
                                    sprite_id: script.sprite_id,
                                    script_name: script.name.clone(),
                                    script,
                                }
                            ));
                            info!("Spawned NPC {} at {},{}", npc_id, x, y);
                        }
                        MapMessage::SpawnMob { mob_id, x, y } => {
                            use crate::core::components::{EntityStats, Position, MobAi, AiState, Velocity};
                            self.world.spawn((
                                EntityStats {
                                    account_id: 0,
                                    char_id: mob_id, // GID (using mob_id for now)
                                    name: format!("Mob_{}", mob_id),
                                    class: mob_id as u16,
                                    base_level: 1,
                                    job_level: 1,
                                    hp: 100,
                                    max_hp: 100,
                                    sp: 0,
                                    max_sp: 0,
                                    str: 1, agi: 1, vit: 1, int: 1, dex: 1, luk: 1, atk: 10, def: 5,
                                    party_id: 0, guild_id: 0,
                                },
                                MobAi {
                                    state: AiState::Idle,
                                    spawn_x: x,
                                    spawn_y: y,
                                    roam_range: 10,
                                    view_range: 12,
                                    is_aggressive: mob_id != 1002,
                                },
                                Position { map_name: self.name.clone(), x, y, dir: 0 },
                                Velocity { speed: 200, target: None, pending_target: None, next_move_tick: std::time::Instant::now() },
                            ));
                            tracing::info!("Spawned mob {} at {},{}", mob_id, x, y);
                        }
                        MapMessage::CommandSpawnMob { char_id, mob_id, amount, respond_to } => {
                            use crate::core::components::{EntityStats, Position};
                            let mut px = 0;
                            let mut py = 0;
                            // Find the player's position
                            let mut query = self.world.query::<(&EntityStats, &Position)>();
                            for (stats, pos) in query.iter(&self.world) {
                                if stats.char_id == char_id {
                                    px = pos.x;
                                    py = pos.y;
                                    break;
                                }
                            }
                            
                            if px != 0 && py != 0 {
                                use rand::RngExt;
                                let mut rng = rand::rng();
                                for _ in 0..amount {
                                    // spawn nearby
                                    let sx = (px as i16 + rng.random_range(-2..=2)).max(0) as u16;
                                    let sy = (py as i16 + rng.random_range(-2..=2)).max(0) as u16;
                                    
                                    let new_id = rng.random_range(200000..300000); // GID
                                    
                                    use crate::core::components::{MobAi, AiState, Velocity};
                                    self.world.spawn((
                                        EntityStats {
                                            account_id: 0,
                                            char_id: new_id,
                                            name: format!("Mob_{}", mob_id),
                                            class: mob_id as u16,
                                            base_level: 1,
                                            job_level: 1,
                                            hp: 100, max_hp: 100, sp: 0, max_sp: 0,
                                            str: 1, agi: 1, vit: 1, int: 1, dex: 1, luk: 1, atk: 10, def: 5,
                                            party_id: 0, guild_id: 0,
                                        },
                                        MobAi {
                                            state: AiState::Idle,
                                            spawn_x: sx, spawn_y: sy,
                                            roam_range: 10, view_range: 12,
                                            is_aggressive: mob_id != 1002,
                                        },
                                        Position { map_name: self.name.clone(), x: sx, y: sy, dir: 0 },
                                        Velocity { speed: 200, target: None, pending_target: None, next_move_tick: std::time::Instant::now() },
                                    ));
                                    tracing::info!("Spawned mob {} at {},{} via @spawn", mob_id, sx, sy);
                                    
                                    // Build correct ZC_NOTIFY_NEWENTRY11 (0x09fe) packet for client (83 bytes)
                                    let mut pkt = vec![0u8; 83];
                                    pkt[0..2].copy_from_slice(&0x09feu16.to_le_bytes());
                                    pkt[2..4].copy_from_slice(&83u16.to_le_bytes());
                                    pkt[4] = 5; // object type = mob (NPC_MOB_TYPE in rAthena)
                                    pkt[5..9].copy_from_slice(&new_id.to_le_bytes()); // AID
                                    pkt[9..13].copy_from_slice(&new_id.to_le_bytes()); // GID
                                    pkt[13..15].copy_from_slice(&200i16.to_le_bytes()); // speed
                                    pkt[23..25].copy_from_slice(&(mob_id as i16).to_le_bytes()); // job/sprite ID
                                    
                                    // Pack position (PosDir) at offset 63
                                    let mut pos_p = [0u8; 3];
                                    pos_p[0] = (sx >> 2) as u8;
                                    pos_p[1] = (((sx << 6) & 0xC0) | ((sy >> 4) & 0x3F)) as u8;
                                    pos_p[2] = (((sy << 4) & 0xF0) | 4) as u8; // dir 4
                                    pkt[63..66].copy_from_slice(&pos_p);
                                    
                                    // max_hp and hp (at offsets 72 and 76)
                                    pkt[72..76].copy_from_slice(&100i32.to_le_bytes());
                                    pkt[76..80].copy_from_slice(&100i32.to_le_bytes());
                                    
                                    // send to packet_tx (under RoPacket, we strip the first 2 bytes of cmd)
                                    let _ = respond_to.send(crate::network::codec::RoPacket {
                                        cmd: 0x09fe,
                                        payload: pkt[2..].to_vec(),
                                    });
                                }
                            }
                        }
                        MapMessage::CommandWarp { char_id, target_map, x, y, respond_to } => {
                            use crate::core::components::{EntityStats, Position};
                            let mut entity_to_update = None;
                            let mut query = self.world.query::<(bevy_ecs::entity::Entity, &EntityStats)>();
                            for (entity, stats) in query.iter(&self.world) {
                                if stats.char_id == char_id {
                                    entity_to_update = Some(entity);
                                    break;
                                }
                            }

                            if let Some(entity) = entity_to_update {
                                if let Some(mut pos) = self.world.get_mut::<Position>(entity) {
                                    pos.map_name = target_map.clone();
                                    pos.x = x;
                                    pos.y = y;
                                }
                                
                                // Send ZC_NPCACK_MAPMOVE (0x0091)
                                    let mut pkt = vec![0u8; 22];
                                    pkt[0] = 0x91;
                                    pkt[1] = 0x00;
                                    
                                    // Map name (16 bytes)
                                    let map_name_bytes = target_map.as_bytes();
                                    let len = map_name_bytes.len().min(15);
                                    pkt[2..2+len].copy_from_slice(&map_name_bytes[..len]);
                                    if !target_map.ends_with(".gat") {
                                        // add .gat if needed by older clients, but rAthena uses mapindex_getmapname_ext
                                        // Usually we just send the map name with .gat appended, but let's just send what was requested
                                    }
                                    
                                    // x (2 bytes)
                                    pkt[18..20].copy_from_slice(&x.to_le_bytes());
                                    // y (2 bytes)
                                    pkt[20..22].copy_from_slice(&y.to_le_bytes());
                                    
                                    let _ = respond_to.send(crate::network::codec::RoPacket {
                                        cmd: 0x0091,
                                        payload: pkt[2..].to_vec(),
                                    });
                                    tracing::info!("Warped char_id {} to {} {},{}", char_id, target_map, x, y);
                            }
                        }
                        MapMessage::Shutdown => {
                            info!("MapActor '{}' shutting down.", self.name);
                            break;
                        }
                    }
                }
                _ = tick_interval.tick() => {
                    // Run ECS systems
                    self.schedule.run(&mut self.world);
                }
            }
        }
    }

    fn handle_script_action(
        world: &mut World,
        char_id: u32,
        npc_id: u32,
        respond_to: tokio::sync::mpsc::UnboundedSender<crate::network::codec::RoPacket>,
        is_start: bool,
        is_next: bool,
        is_close: bool,
        menu_selection: Option<u8>
    ) {
        use crate::core::components::{EntityStats, Npc, ScriptSession};
        use crate::script::{ScriptContext, ScriptCommand};

        // Find Player
        let mut query = world.query::<(Entity, &EntityStats)>();
        let mut player_ent = None;
        for (e, stats) in query.iter(world) {
            if stats.char_id == char_id {
                player_ent = Some(e);
                break;
            }
        }
        
        let player_ent = match player_ent {
            Some(e) => e,
            None => return,
        };

        if is_start {
            // Find NPC script
            let mut npc_script = None;
            let mut npc_query = world.query::<(Entity, &EntityStats, &Npc)>();
            for (_e, stats, npc) in npc_query.iter(world) {
                if stats.char_id == npc_id {
                    npc_script = Some(npc.script.clone());
                    break;
                }
            }
            
            if let Some(script) = npc_script {
                world.entity_mut(player_ent).insert(ScriptSession {
                    context: ScriptContext::new(char_id, npc_id, script),
                });
            } else {
                return; // NPC not found
            }
        }

        // Run Script
        let mut session = match world.get_mut::<ScriptSession>(player_ent) {
            Some(s) => s,
            None => return,
        };
        
        if is_close {
            world.entity_mut(player_ent).remove::<ScriptSession>();
            return;
        }
        
        if let Some(sel) = menu_selection {
            // Find the menu we were at (current_line - 1)
            let current = session.context.current_line;
            if current > 0 {
                if let ScriptCommand::Menu(options) = &session.context.script.commands[current - 1] {
                    if (sel as usize) > 0 && (sel as usize) <= options.len() {
                        let target_line = options[(sel as usize) - 1].1;
                        session.context.current_line = target_line;
                    }
                }
            }
        }

        let outputs = session.context.run_until_yield();
        
        // Send packets
        for cmd in outputs {
            match cmd {
                ScriptCommand::Mes(text) => {
                    let mut msg_bytes = text.into_bytes();
                    msg_bytes.push(0); // null terminator
                    let mut reply = vec![0u8; 8 + msg_bytes.len()];
                    reply[0] = 0xb4;
                    reply[1] = 0x00;
                    let len = reply.len() as u16;
                    reply[2..4].copy_from_slice(&len.to_le_bytes());
                    reply[4..8].copy_from_slice(&npc_id.to_le_bytes());
                    reply[8..].copy_from_slice(&msg_bytes);
                    let _ = respond_to.send(crate::network::codec::RoPacket { cmd: 0x00b4, payload: reply[2..].to_vec() });
                }
                ScriptCommand::Next => {
                    let mut reply = vec![0u8; 6];
                    reply[0] = 0xb5;
                    reply[1] = 0x00;
                    reply[2..6].copy_from_slice(&npc_id.to_le_bytes());
                    let _ = respond_to.send(crate::network::codec::RoPacket { cmd: 0x00b5, payload: reply[2..].to_vec() });
                }
                ScriptCommand::Heal(hp, sp) => {
                    // Update HP/SP
                    tracing::info!("Healed {}, {}", hp, sp);
                }
                ScriptCommand::GetItem(item_id, amount) => {
                    if let Some(mut inv) = world.get_mut::<crate::core::components::Inventory>(player_ent) {
                        inv.add_item(item_id, amount);
                        tracing::info!("Player {} received {}x item {}", char_id, amount, item_id);
                    }
                }
                ScriptCommand::DelItem(item_id, amount) => {
                    if let Some(mut inv) = world.get_mut::<crate::core::components::Inventory>(player_ent) {
                        inv.remove_item(item_id, amount);
                        tracing::info!("Player {} lost {}x item {}", char_id, amount, item_id);
                    }
                }
                ScriptCommand::SetZeny(amount) => {
                    if let Some(mut inv) = world.get_mut::<crate::core::components::Inventory>(player_ent) {
                        inv.zeny = amount as u32; // Assuming amount is positive for now
                        tracing::info!("Player {} zeny set to {}", char_id, amount);
                    }
                }
                ScriptCommand::Close => {
                    let mut reply = vec![0u8; 6];
                    reply[0] = 0xb6;
                    reply[1] = 0x00;
                    reply[2..6].copy_from_slice(&npc_id.to_le_bytes());
                    let _ = respond_to.send(crate::network::codec::RoPacket { cmd: 0x00b6, payload: reply[2..].to_vec() });
                }
                ScriptCommand::Menu(options) => {
                    let menu_str = options.iter().map(|(label, _)| label.as_str()).collect::<Vec<&str>>().join(":");
                    let mut msg_bytes = menu_str.into_bytes();
                    msg_bytes.push(0); // null terminator
                    let mut reply = vec![0u8; 8 + msg_bytes.len()];
                    reply[0] = 0xb7;
                    reply[1] = 0x00;
                    let len = reply.len() as u16;
                    reply[2..4].copy_from_slice(&len.to_le_bytes());
                    reply[4..8].copy_from_slice(&npc_id.to_le_bytes());
                    reply[8..].copy_from_slice(&msg_bytes);
                    let _ = respond_to.send(crate::network::codec::RoPacket { cmd: 0x00b7, payload: reply[2..].to_vec() });
                }
                ScriptCommand::Warp(map, x, y) => {
                    // Send ZC_NPCACK_MAPMOVE (0x008f) or similar? 
                    // Usually we need to tell the player to change map.
                    // For now, print a warning since real map changing needs more coordination.
                    tracing::info!("Warp requested to {} at {},{}", map, x, y);
                    // Just close the dialog
                    let mut reply = vec![0u8; 6];
                    reply[0] = 0xb6;
                    reply[1] = 0x00;
                    reply[2..6].copy_from_slice(&npc_id.to_le_bytes());
                    let _ = respond_to.send(crate::network::codec::RoPacket { cmd: 0x00b6, payload: reply[2..].to_vec() });
                }
                _ => {} // Heal is internal
            }
        }
    }
}

pub struct MapManager {
    pub maps: HashMap<String, mpsc::Sender<MapMessage>>,
    pub server_state: Arc<crate::core::state::ServerState>,
}

impl MapManager {
    pub fn new(server_state: Arc<crate::core::state::ServerState>) -> Self {
        Self {
            maps: HashMap::new(),
            server_state,
        }
    }

    pub fn start_map(&mut self, name: &str) {
        let (tx, rx) = mpsc::channel(100);
        let mut actor = MapActor::new(name.to_string(), rx, self.server_state.clone());
        
        tokio::spawn(async move {
            actor.run().await;
        });

        self.maps.insert(name.to_string(), tx);
    }

    pub async fn shutdown_all(&self) {
        for (name, tx) in &self.maps {
            if let Err(e) = tx.send(MapMessage::Shutdown).await {
                warn!("Failed to send shutdown to map {}: {}", name, e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::components::{EntityStats, Position, Inventory, GroundItem};
    use bevy_ecs::prelude::*;

    #[test]
    fn test_inventory_drop_and_pickup() {
        let mut world = World::new();

        let mut inv = Inventory::new();
        inv.add_item(501, 10); // Red Potion x 10

        let player = world.spawn((
            EntityStats {
                account_id: 1, char_id: 150000, name: "Test".into(), class: 0,
                base_level: 1, job_level: 1, hp: 10, max_hp: 10, sp: 10, max_sp: 10,
                str: 1, agi: 1, vit: 1, int: 1, dex: 1, luk: 1, atk: 10, def: 10,
                party_id: 0, guild_id: 0,
            },
            Position { map_name: "prontera".into(), x: 100, y: 100, dir: 4 },
            inv,
        )).id();
        
        let mut drop_pos = None;
        let char_id = 150000;
        let drop_item_id = 501;
        let drop_amount = 3;
        
        // Simulating DropItem handler
        let mut query = world.query::<(&EntityStats, &mut Inventory, &Position)>();
        for (stats, mut inv, pos) in query.iter_mut(&mut world) {
            if stats.char_id == char_id {
                if inv.remove_item(drop_item_id, drop_amount) {
                    drop_pos = Some((pos.x, pos.y, pos.map_name.clone()));
                }
            }
        }
        
        assert!(drop_pos.is_some());
        
        let mut dropped_entity_id = 0;
        if let Some((x, y, map_name)) = drop_pos {
            dropped_entity_id = world.spawn((
                Position { map_name, x, y, dir: 0 },
                GroundItem {
                    item_id: drop_item_id,
                    amount: drop_amount,
                    dropped_by: char_id,
                    drop_time: std::time::Instant::now(),
                },
            )).id().to_bits() as u32;
        }

        // Verify Inventory has 7 potions left
        let inv = world.get::<Inventory>(player).unwrap();
        assert_eq!(inv.items[0].amount, 7);
        
        // Verify GroundItem exists
        let mut ground_query = world.query::<(Entity, &GroundItem)>();
        assert_eq!(ground_query.iter(&world).count(), 1);
        
        // Simulating PickupItem handler
        let ground_entity_id = dropped_entity_id;
        let mut item_to_give = None;
        let mut entity_to_despawn = None;
        
        for (entity, g_item) in ground_query.iter(&world) {
            if (entity.to_bits() as u32) == ground_entity_id {
                item_to_give = Some((g_item.item_id, g_item.amount));
                entity_to_despawn = Some(entity);
            }
        }
        
        if let (Some(entity), Some((i_id, amt))) = (entity_to_despawn, item_to_give) {
            let mut inv_query = world.query::<(&EntityStats, &mut Inventory)>();
            let mut found = false;
            for (stats, mut inv) in inv_query.iter_mut(&mut world) {
                if stats.char_id == char_id {
                    inv.add_item(i_id, amt);
                    found = true;
                }
            }
            if found {
                world.despawn(entity);
            }
        }
        
        // Verify Inventory has 10 potions again
        let inv = world.get::<Inventory>(player).unwrap();
        assert_eq!(inv.items[0].amount, 10);
        
        // Verify GroundItem is gone
        assert_eq!(world.query::<&GroundItem>().iter(&world).count(), 0);
    }

    #[test]
    fn test_battle_and_ai() {
        use crate::core::components::{AttackTarget, NextAttackTick, MobAi, AiState};
        let mut world = World::new();

        let player_char_id = 150000;
        let mob_char_id = 1002; // Poring

        let player = world.spawn((
            EntityStats {
                account_id: 1, char_id: player_char_id, name: "Test".into(), class: 0,
                base_level: 99, job_level: 50, hp: 1000, max_hp: 1000, sp: 100, max_sp: 100,
                str: 100, agi: 1, vit: 1, int: 1, dex: 100, luk: 1, atk: 50, def: 5,
                party_id: 0, guild_id: 0,
            },
            Position { map_name: "prontera".into(), x: 100, y: 100, dir: 4 },
        )).id();

        let mob = world.spawn((
            EntityStats {
                account_id: 0, char_id: mob_char_id, name: "Mob_1002".into(), class: 1002,
                base_level: 1, job_level: 1, hp: 30, max_hp: 30, sp: 0, max_sp: 0,
                str: 1, agi: 1, vit: 1, int: 1, dex: 1, luk: 1, atk: 5, def: 2,
                party_id: 0, guild_id: 0,
            },
            MobAi {
                state: AiState::Idle,
                spawn_x: 100,
                spawn_y: 100,
                roam_range: 10,
                view_range: 12,
            },
            Velocity { speed: 200, target: None, pending_target: None, next_move_tick: std::time::Instant::now() },
            Position { map_name: "prontera".into(), x: 100, y: 100, dir: 4 },
        )).id();

        // Simulate PlayerAttack message handling
        world.entity_mut(player).insert(AttackTarget(mob_char_id));
        
        // Use a time in the past so the attack triggers immediately
        let past = std::time::Instant::now() - std::time::Duration::from_secs(5);
        world.entity_mut(player).insert(NextAttackTick(past));

        // Create a basic schedule to run just the battle system
        let mut schedule = Schedule::default();
        schedule.add_systems(crate::core::systems::battle_system);

        // Run battle system (Player hits Mob)
        schedule.run(&mut world);

        // Mob only has 30 HP, Player has massive ATK (STR 100). It should die in 1 hit.
        assert!(world.get::<EntityStats>(mob).is_none());
    }

    #[test]
    fn test_skill_casting_and_damage() {
        let mut world = World::new();
        
        let player_char_id = 150000;
        let mob_char_id = 2000000;
        
        // Spawn Player
        let player = world.spawn((
            EntityStats {
                account_id: 1, char_id: player_char_id, name: "Mage".into(), class: 9, // Mage
                base_level: 50, job_level: 50, hp: 1000, max_hp: 1000, sp: 500, max_sp: 500,
                str: 1, agi: 1, vit: 1, int: 99, dex: 99, luk: 1, atk: 10, def: 5,
                party_id: 0, guild_id: 0,
            },
            Position { map_name: "prontera".into(), x: 100, y: 100, dir: 4 },
        )).id();

        // Spawn Mob (Poring)
        let mob = world.spawn((
            EntityStats {
                account_id: 0, char_id: mob_char_id, name: "Poring".into(), class: 1002,
                base_level: 1, job_level: 1, hp: 50, max_hp: 50, sp: 0, max_sp: 0,
                str: 1, agi: 1, vit: 1, int: 1, dex: 1, luk: 1, atk: 5, def: 2,
                party_id: 0, guild_id: 0,
            },
            Position { map_name: "prontera".into(), x: 100, y: 100, dir: 4 },
        )).id();

        // Player casts Fire Bolt level 10
        let past = std::time::Instant::now() - std::time::Duration::from_secs(10); // Instant finish
        world.entity_mut(player).insert(crate::core::components::SkillCasting {
            skill_id: 19, // Fire Bolt
            skill_level: 10,
            target: crate::core::components::SkillTarget::Entity(mob_char_id),
            cast_time: 8000,
            start_tick: past,
            skill_type: crate::database::skill::SkillType::Magic,
            multiplier: 10.0,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(crate::core::systems::skill_system);

        schedule.run(&mut world);

        // Mage with INT 99 using FireBolt Lv10 (10x multiplier) does massive damage
        // The mob should die immediately
        assert!(world.get::<EntityStats>(mob).is_none());
        assert!(world.get::<crate::core::components::SkillCasting>(player).is_none()); // Casting removed
    }

    #[test]
    fn test_healing_and_buffs() {
        let mut world = World::new();
        
        let player_char_id = 150000;
        let ally_char_id = 150001;
        
        // Spawn Aco
        let aco = world.spawn((
            EntityStats {
                account_id: 1, char_id: player_char_id, name: "Aco".into(), class: 8,
                base_level: 50, job_level: 50, hp: 1000, max_hp: 1000, sp: 500, max_sp: 500,
                str: 1, agi: 1, vit: 1, int: 99, dex: 1, luk: 1, atk: 10, def: 5,
                party_id: 1, guild_id: 0,
            },
            Position { map_name: "prontera".into(), x: 100, y: 100, dir: 4 },
        )).id();

        // Spawn Ally (injured)
        let ally = world.spawn((
            EntityStats {
                account_id: 1, char_id: ally_char_id, name: "Knight".into(), class: 7,
                base_level: 99, job_level: 50, hp: 100, max_hp: 10000, sp: 50, max_sp: 50,
                str: 99, agi: 1, vit: 99, int: 1, dex: 1, luk: 1, atk: 100, def: 50,
                party_id: 1, guild_id: 0,
            },
            Position { map_name: "prontera".into(), x: 100, y: 100, dir: 4 },
        )).id();

        // Aco casts Heal level 10 on Ally
        let past = std::time::Instant::now() - std::time::Duration::from_secs(1);
        world.entity_mut(aco).insert(crate::core::components::SkillCasting {
            skill_id: 28, // Heal
            skill_level: 10,
            target: crate::core::components::SkillTarget::Entity(ally_char_id),
            cast_time: 0,
            start_tick: past,
            skill_type: crate::database::skill::SkillType::Heal,
            multiplier: 0.0,
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(crate::core::systems::skill_system);
        schedule.run(&mut world);

        // Verify heal
        let ally_stats = world.get::<EntityStats>(ally).unwrap();
        assert!(ally_stats.hp > 100); // HP should increase significantly
        assert!(world.get::<crate::core::components::SkillCasting>(aco).is_none());
    }
}
