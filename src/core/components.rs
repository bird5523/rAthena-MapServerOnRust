use bevy_ecs::prelude::*;
use tokio::sync::mpsc::Sender;
use crate::network::codec::RoPacket;

#[derive(Component, Clone)]
pub struct PlayerConnection {
    pub tx: tokio::sync::mpsc::UnboundedSender<RoPacket>,
}

#[derive(Component, Debug, Clone)]
pub struct Position {
    pub map_name: String,
    pub x: u16,
    pub y: u16,
    pub dir: u8,
}

#[derive(Component, Debug, Clone)]
pub struct Velocity {
    pub speed: u16,
    pub target: Option<(u16, u16)>, // Target X, Y
    pub pending_target: Option<(u16, u16)>, // Queued target while mid-step
    pub next_move_tick: std::time::Instant, // Throttle based on speed
}

#[derive(Component, Debug, Clone)]
pub struct EntityStats {
    pub account_id: u32,
    pub char_id: u32,
    pub name: String,
    pub class: u16,
    pub base_level: u16,
    pub job_level: u16,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub str: u16,
    pub agi: u16,
    pub vit: u16,
    pub int: u16,
    pub dex: u16,
    pub luk: u16,
    pub atk: u16,
    pub def: u16,
    pub party_id: u32,
    pub guild_id: u32,
}

#[derive(Component, Debug, Clone)]
pub struct AttackTarget(pub u32); // char_id of the target

#[derive(Component, Debug, Clone)]
pub struct NextAttackTick(pub std::time::Instant);

#[derive(Component)]
pub struct NetworkClient {
    pub tx: Sender<RoPacket>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryItem {
    pub item_id: i32,
    pub amount: u16,
    pub equip_location: u32, // 0 if not equipped
    pub identify: bool,
}

#[derive(Component, Debug, Clone)]
pub struct Inventory {
    pub items: Vec<InventoryItem>,
    pub zeny: u32,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            zeny: 0,
        }
    }

    pub fn add_item(&mut self, item_id: i32, amount: u16) {
        if let Some(existing) = self.items.iter_mut().find(|i| i.item_id == item_id && i.equip_location == 0) {
            existing.amount = existing.amount.saturating_add(amount);
        } else {
            self.items.push(InventoryItem {
                item_id,
                amount,
                equip_location: 0,
                identify: true, // Auto-identify for now
            });
        }
    }

    pub fn remove_item(&mut self, item_id: i32, amount: u16) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i.item_id == item_id && i.equip_location == 0) {
            if self.items[pos].amount > amount {
                self.items[pos].amount -= amount;
                true
            } else if self.items[pos].amount == amount {
                self.items.remove(pos);
                true
            } else {
                false // Not enough amount
            }
        } else {
            false
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct GroundItem {
    pub item_id: i32,
    pub amount: u16,
    pub dropped_by: u32, // Entity ID that dropped it
    pub drop_time: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiState {
    Idle,
    Roaming,
    Chasing(u32), // Chasing target entity ID
    Attacking(u32), // Attacking target entity ID
    Dead,
}

#[derive(Component, Debug, Clone)]
pub struct MobAi {
    pub state: AiState,
    pub spawn_x: u16,
    pub spawn_y: u16,
    pub roam_range: u16,
    pub view_range: u16,
    pub is_aggressive: bool,
}

#[derive(Component, Debug, Clone)]
pub struct Npc {
    pub sprite_id: u16,
    pub script_name: String,
    pub script: crate::script::NpcScript, // The attached script
}

#[derive(Component, Debug, Clone)]
pub struct ScriptSession {
    pub context: crate::script::ScriptContext,
}

#[derive(Clone, Debug)]
pub enum SkillTarget {
    Entity(u32), // Target entity ID
    Ground(u16, u16), // Target X, Y coordinates
}

#[derive(Component, Debug, Clone)]
pub struct SkillCasting {
    pub skill_id: u16,
    pub skill_level: u16,
    pub target: SkillTarget,
    pub cast_time: u32, // Total cast time in ms
    pub start_tick: std::time::Instant, // When casting started
    pub skill_type: crate::database::skill::SkillType,
    pub multiplier: f32,
}

#[derive(Component, Debug, Clone)]
pub struct SkillCooldowns {
    pub cooldowns: std::collections::HashMap<u16, std::time::Instant>, // skill_id -> ready time
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatusEffectType {
    Stun,
    Poison,
    Blessing,
    AgiUp,
}

#[derive(Debug, Clone)]
pub struct ActiveStatus {
    pub effect_type: StatusEffectType,
    pub end_tick: std::time::Instant,
    pub value: i32,
    pub next_tick: Option<std::time::Instant>, // For Damage over time (e.g., Poison)
}

#[derive(Component, Debug, Clone, Default)]
pub struct StatusEffects {
    pub effects: Vec<ActiveStatus>,
}

