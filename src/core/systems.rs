use bevy_ecs::prelude::*;
use rand::Rng;
use crate::core::components::*;
use crate::network::codec::RoPacket;
use tracing::info;

pub fn movement_system(
    mut query: Query<(Entity, &mut Position, &mut Velocity, Option<&EntityStats>, Option<&MobAi>)>,
    player_query: Query<&PlayerConnection>,
) {
    let now = std::time::Instant::now();
    let mut movements = Vec::new();

    for (entity, mut pos, mut vel, stats_opt, mob_opt) in query.iter_mut() {
        if let Some((target_x, target_y)) = vel.target {
            if now < vel.next_move_tick {
                continue;
            }
            
            let old_x = pos.x;
            let old_y = pos.y;
            
            // Very simple step movement (not fully aligned with RO timing yet)
            if pos.x < target_x { pos.x += 1; }
            else if pos.x > target_x { pos.x -= 1; }
            
            if pos.y < target_y { pos.y += 1; }
            else if pos.y > target_y { pos.y -= 1; }
            
            if pos.x != old_x || pos.y != old_y {
                if mob_opt.is_some() {
                    let gid = stats_opt.map(|s| s.char_id).unwrap_or(entity.to_bits() as u32);
                    movements.push((gid, old_x, old_y, pos.x, pos.y));
                }
            }
            
            if pos.x == target_x && pos.y == target_y {
                if let Some(next_target) = vel.pending_target.take() {
                    vel.target = Some(next_target);
                    vel.next_move_tick = now + std::time::Duration::from_millis(vel.speed as u64);
                } else {
                    vel.target = None;
                }
            } else {
                vel.next_move_tick = now + std::time::Duration::from_millis(vel.speed as u64);
                // Even if we haven't reached the final target, if the user clicked somewhere else,
                // we apply it now! This perfectly mirrors rAthena's unit_walktoxy_sub behavior!
                if let Some(next_target) = vel.pending_target.take() {
                    vel.target = Some(next_target);
                }
            }
        }
    }

    // Broadcast movements to all players
    for (gid, x0, y0, x1, y1) in movements {
        // Send ZC_NOTIFY_MOVE (0x0086)
        let mut pkt = vec![0u8; 16];
        pkt[0] = 0x86;
        pkt[1] = 0x00;
        pkt[2..6].copy_from_slice(&gid.to_le_bytes());
        
        // Pack coordinates
        pkt[6] = (x0 >> 2) as u8;
        pkt[7] = (((x0 << 6) & 0xC0) | ((y0 >> 4) & 0x3F)) as u8;
        pkt[8] = (((y0 << 4) & 0xF0) | ((x1 >> 6) & 0x0F)) as u8;
        pkt[9] = (((x1 << 2) & 0xFC) | ((y1 >> 8) & 0x03)) as u8;
        pkt[10] = y1 as u8;
        pkt[11] = 0x88; // sx0=8, sy0=8
        
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u32;
        pkt[12..16].copy_from_slice(&tick.to_le_bytes());
        
        for conn in player_query.iter() {
            let _ = conn.tx.send(RoPacket {
                cmd: 0x0086,
                payload: pkt[2..].to_vec(),
            });
        }
    }
}

pub fn ai_system(
    mut commands: Commands,
    mut ai_query: Query<(Entity, &mut MobAi, &mut Velocity, &Position)>,
    target_query: Query<(&EntityStats, &Position, Option<&PlayerConnection>)>,
) {
    for (entity, mut ai, mut vel, pos) in ai_query.iter_mut() {
        match ai.state {
            AiState::Idle | AiState::Roaming => {
                // Check for targets in view range if aggressive
                let mut found_target = None;
                if ai.is_aggressive {
                    for (stats, t_pos, conn_opt) in target_query.iter() {
                        if conn_opt.is_some() {
                            let dist = (pos.x as i32 - t_pos.x as i32).abs() + (pos.y as i32 - t_pos.y as i32).abs();
                            if dist <= ai.view_range as i32 {
                                found_target = Some(stats.char_id);
                                break;
                            }
                        }
                    }
                }

                if let Some(target_id) = found_target {
                    ai.state = AiState::Chasing(target_id);
                    tracing::info!("Mob started chasing {}", target_id);
                } else if ai.state == AiState::Idle {
                    // Randomly start roaming
                    if rand::random::<f32>() < 0.05 {
                        ai.state = AiState::Roaming;
                        let dx = (rand::random::<i16>() % 10) - 5;
                        let dy = (rand::random::<i16>() % 10) - 5;
                        vel.target = Some(((ai.spawn_x as i16 + dx) as u16, (ai.spawn_y as i16 + dy) as u16));
                    }
                } else if ai.state == AiState::Roaming && vel.target.is_none() {
                    ai.state = AiState::Idle;
                }
            }
            AiState::Chasing(target_id) => {
                let mut target_pos = None;
                for (stats, t_pos, _) in target_query.iter() {
                    if stats.char_id == target_id {
                        target_pos = Some((t_pos.x, t_pos.y));
                        break;
                    }
                }

                if let Some((tx, ty)) = target_pos {
                    let dist = (pos.x as i32 - tx as i32).abs() + (pos.y as i32 - ty as i32).abs();
                    if dist <= 1 {
                        // In melee range!
                        ai.state = AiState::Attacking(target_id);
                        vel.target = None;
                        commands.entity(entity).insert(AttackTarget(target_id));
                        commands.entity(entity).insert(NextAttackTick(std::time::Instant::now()));
                    } else if dist > ai.view_range as i32 * 2 {
                        // Lost target
                        ai.state = AiState::Idle;
                        vel.target = None;
                    } else {
                        // Keep chasing
                        vel.target = Some((tx, ty));
                    }
                } else {
                    // Target disappeared
                    ai.state = AiState::Idle;
                    vel.target = None;
                }
            }
            AiState::Attacking(target_id) => {
                let mut target_pos = None;
                for (stats, t_pos, _) in target_query.iter() {
                    if stats.char_id == target_id {
                        target_pos = Some((t_pos.x, t_pos.y));
                        break;
                    }
                }

                if let Some((tx, ty)) = target_pos {
                    let dist = (pos.x as i32 - tx as i32).abs() + (pos.y as i32 - ty as i32).abs();
                    if dist > 1 {
                        // Target moved away, chase again
                        ai.state = AiState::Chasing(target_id);
                        commands.entity(entity).remove::<AttackTarget>();
                        commands.entity(entity).remove::<NextAttackTick>();
                    }
                } else {
                    // Target disappeared
                    ai.state = AiState::Idle;
                    commands.entity(entity).remove::<AttackTarget>();
                    commands.entity(entity).remove::<NextAttackTick>();
                }
            }
            _ => {}
        }
    }
}

pub fn battle_system(
    mut commands: Commands,
    global_state: Option<Res<crate::core::state::GlobalState>>,
    mut query: Query<(Entity, &mut EntityStats, &mut Position, Option<&mut MobAi>, Option<&AttackTarget>, Option<&mut NextAttackTick>)>,
    player_query: Query<&PlayerConnection>,
) {
    let now = std::time::Instant::now();
    let mut damages = Vec::new(); // (attacker_char_id, target_char_id, damage, is_crit, is_miss)
    let mut intents = Vec::new();

    // Collect all positions first to avoid double borrow of query
    let mut positions = std::collections::HashMap::new();
    for (_, stats, pos, _, _, _) in query.iter() {
        positions.insert(stats.char_id, (pos.x, pos.y));
    }

    // Phase 1: Collect attacks
    for (_entity, stats, pos, _mob_opt, target_opt, tick_opt) in query.iter_mut() {
        if let (Some(target), Some(mut next_tick)) = (target_opt, tick_opt) {
            if now >= next_tick.0 {
                if let Some(&(tx, ty)) = positions.get(&target.0) {
                    let dist = (pos.x as i32 - tx as i32).abs() + (pos.y as i32 - ty as i32).abs();
                    if dist <= 1 {
                        intents.push((stats.clone(), target.0));
                        next_tick.0 = now + std::time::Duration::from_millis(1000);
                    }
                }
            }
        }
    }

    // Phase 1.5: Calculate damages (avoids double borrow of query)
    for (attacker_stats, target_id) in intents {
        let mut defender_stats = None;
        for (_, t_stats, _, _, _, _) in query.iter() {
            if t_stats.char_id == target_id {
                defender_stats = Some(t_stats.clone());
                break;
            }
        }
        
        if let Some(def) = defender_stats {
            let dmg_result = crate::core::combat::calculate_melee_damage(&attacker_stats, &def);
            damages.push((attacker_stats.char_id, target_id, dmg_result.damage, dmg_result.is_crit, dmg_result.is_miss));
        }
    }

    // Phase 2: Apply damages
    for (attacker_id, target_id, dmg, is_crit, is_miss) in damages {
        if is_miss {
            continue;
        }
        for (entity, mut stats, mut pos, mut mob_opt, _, _) in query.iter_mut() {
            if stats.char_id == target_id {
                stats.hp = stats.hp.saturating_sub(dmg);
                let crit_str = if is_crit { " (CRIT!)" } else { "" };
                tracing::info!("Entity {} took {}{} damage, HP is now {}", stats.name, dmg, crit_str, stats.hp);
                
                // Broadcast attack animation (ZC_NOTIFY_ACT 0x008a) to all players!
                let mut act_pkt = vec![0u8; 29];
                act_pkt[0..2].copy_from_slice(&0x008au16.to_le_bytes());
                act_pkt[2..6].copy_from_slice(&attacker_id.to_le_bytes()); // srcAID
                act_pkt[6..10].copy_from_slice(&target_id.to_le_bytes()); // targetAID
                
                let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u32;
                act_pkt[10..14].copy_from_slice(&tick.to_le_bytes()); // actionStartTime
                
                act_pkt[14..18].copy_from_slice(&300i32.to_le_bytes()); // actionDelayTime
                act_pkt[18..22].copy_from_slice(&(dmg as i32).to_le_bytes()); // damage
                act_pkt[22..24].copy_from_slice(&1i16.to_le_bytes()); // count (1 hit)
                
                let action_type: u8 = if is_crit { 3 } else { 1 }; // action (1 = normal hit, 3 = critical hit)
                act_pkt[24] = action_type;
                
                for conn in player_query.iter() {
                    let _ = conn.tx.send(RoPacket {
                        cmd: 0x008a,
                        payload: act_pkt[2..].to_vec(),
                    });
                }
                
                if stats.hp == 0 {
                    if stats.account_id != 0 {
                        stats.hp = stats.max_hp;
                        pos.x = 53;
                        pos.y = 111;
                        tracing::info!("Player {} died and was resurrected at 53,111!", stats.name);
                    } else {
                        handle_entity_death(&mut commands, entity, &stats, &pos, global_state.as_deref());
                    }
                } else {
                    // Retaliate if it is a passive mob that was hit
                    if let Some(ref mut mob_ai) = mob_opt {
                        match mob_ai.state {
                            AiState::Idle | AiState::Roaming => {
                                mob_ai.state = AiState::Chasing(attacker_id);
                                tracing::info!("Passive mob {} was hit! Retaliating against attacker {}", stats.name, attacker_id);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

pub fn skill_system(
    mut commands: Commands,
    global_state: Option<Res<crate::core::state::GlobalState>>,
    mut query: Query<(Entity, &mut crate::core::components::EntityStats, &Position, Option<&crate::core::components::SkillCasting>)>
) {
    let now = std::time::Instant::now();
    
    // Phase 1: Collect finished casts
    let mut finished_casts = Vec::new();
    for (entity, stats, _pos, casting_opt) in query.iter() {
        if let Some(casting) = casting_opt {
            if now.duration_since(casting.start_tick).as_millis() as u32 >= casting.cast_time {
                finished_casts.push((
                    entity,
                    stats.clone(),
                    casting.clone()
                ));
            }
        }
    }

    // Phase 2: Calculate effects
    let mut apply_heals = Vec::new();
    let mut apply_damages = Vec::new();

    for (caster_entity, caster_stats, casting) in finished_casts {
        // Remove casting state
        commands.entity(caster_entity).remove::<crate::core::components::SkillCasting>();

        // For now we assume target is a character ID (Entity variant)
        if let crate::core::components::SkillTarget::Entity(target_id) = casting.target {
            
            // Find target
            let mut target_stats_opt = None;
            for (_, stats, _pos, _) in query.iter() {
                if stats.char_id == target_id {
                    target_stats_opt = Some(stats.clone());
                    break;
                }
            }

            if let Some(target_stats) = target_stats_opt {
                // Calculate skill effect based on type
                let dummy_skill = crate::database::skill::SkillModel {
                    id: casting.skill_id,
                    name: "Temp".into(),
                    skill_type: casting.skill_type.clone(),
                    max_level: 10,
                    sp_cost: vec![0; 10],
                    cast_time: vec![0; 10],
                    cast_delay: vec![0; 10],
                    range: vec![0; 10],
                    damage_multiplier: vec![casting.multiplier; 10],
                };

                let result = crate::core::combat::calculate_skill_damage(&caster_stats, &target_stats, &dummy_skill, casting.skill_level);

                if casting.skill_type == crate::database::skill::SkillType::Heal {
                    apply_heals.push((target_id, result.damage));
                    tracing::info!("{} heals {} for {}", caster_stats.name, target_stats.name, result.damage);
                } else if casting.skill_type == crate::database::skill::SkillType::Physical || casting.skill_type == crate::database::skill::SkillType::Magic {
                    apply_damages.push((target_id, result.damage));
                    tracing::info!("{} hits {} with skill for {}", caster_stats.name, target_stats.name, result.damage);
                }
            }
        }
    }

    // Phase 3: Apply changes
    for (target_id, heal_amount) in apply_heals {
        for (_, mut stats, _pos, _) in query.iter_mut() {
            if stats.char_id == target_id {
                stats.hp = (stats.hp + heal_amount).min(stats.max_hp);
            }
        }
    }

    for (target_id, damage_amount) in apply_damages {
        for (entity, mut stats, pos, _) in query.iter_mut() {
            if stats.char_id == target_id {
                stats.hp = stats.hp.saturating_sub(damage_amount);
                if stats.hp == 0 {
                    handle_entity_death(&mut commands, entity, &stats, pos, global_state.as_deref());
                }
            }
        }
    }
}

pub fn status_effect_system(
    mut commands: Commands,
    global_state: Option<Res<crate::core::state::GlobalState>>,
    mut query: Query<(Entity, &mut crate::core::components::EntityStats, &Position, &mut crate::core::components::StatusEffects)>
) {
    let now = std::time::Instant::now();
    for (entity, mut stats, pos, mut statuses) in query.iter_mut() {
        // Retain only effects that have not expired
        statuses.effects.retain(|effect| now < effect.end_tick);
        
        // Process DoT (Poison)
        for effect in statuses.effects.iter_mut() {
            if effect.effect_type == crate::core::components::StatusEffectType::Poison {
                if let Some(next_tick) = effect.next_tick {
                    if now >= next_tick {
                        let damage = (stats.max_hp as f32 * 0.01) as u32; // 1% max hp poison per tick
                        stats.hp = stats.hp.saturating_sub(damage.max(1));
                        effect.next_tick = Some(now + std::time::Duration::from_millis(1000));
                        tracing::info!("{} took poison damage: {}", stats.name, damage);
                        if stats.hp == 0 {
                            handle_entity_death(&mut commands, entity, &stats, pos, global_state.as_deref());
                            break;
                        }
                    }
                }
            }
        }
    }
}

pub fn handle_entity_death(
    commands: &mut Commands,
    entity: Entity,
    stats: &EntityStats,
    pos: &Position,
    global_state: Option<&crate::core::state::GlobalState>,
) {
    tracing::info!("Entity {} died!", stats.name);
    
    if let Some(state) = global_state {
        // If it's a mob (account_id == 0 is our convention)
        if stats.account_id == 0 {
            if let Some(mob_data) = state.0.db_manager.mobs.mobs.get(&(stats.class as i32)) {
                for drop in &mob_data.drops {
                    let roll = (rand::random::<u32>() % 10000) as i32;
                    if roll < drop.rate {
                        // Look up item_id by name
                        if let Some(&item_id) = state.0.db_manager.items.name_to_id.get(&drop.item) {
                            tracing::info!("Dropped item {} (ID: {}) at {:?}", drop.item, item_id, pos);
                            commands.spawn((
                                crate::core::components::GroundItem {
                                    item_id,
                                    amount: 1,
                                    dropped_by: 0,
                                    drop_time: std::time::Instant::now(),
                                },
                                pos.clone(),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Despawn the dead entity
    commands.entity(entity).despawn();
}
