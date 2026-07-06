use crate::core::components::EntityStats;

pub struct DamageResult {
    pub damage: u32,
    pub is_crit: bool,
    pub is_miss: bool,
}

pub fn calculate_melee_damage(attacker: &EntityStats, defender: &EntityStats) -> DamageResult {
    // Basic RO formula approximation for melee
    // ATK = STR + (STR/10)^2 + (DEX/5) + (LUK/5)
    let str = attacker.str as u32;
    let dex = attacker.dex as u32;
    let luk = attacker.luk as u32;
    
    let base_atk = str + (str / 10).pow(2) + (dex / 5) + (luk / 5);
    
    // Weapon ATK is assumed to be 0 for now (bare hands)
    let weapon_atk = 10; 
    let mut total_atk = base_atk + weapon_atk;
    
    // Variance
    let variance = (weapon_atk as f32 * 0.1) as u32;
    if variance > 0 {
        total_atk += (rand::random::<u32>() % variance) - (variance / 2);
    }
    
    // Hit/Flee calculation
    // HIT = Level + DEX
    // FLEE = Level + AGI
    let hit = attacker.base_level as u32 + dex;
    let flee = defender.base_level as u32 + defender.agi as u32;
    
    let mut hit_chance = 80 + hit as i32 - flee as i32;
    hit_chance = hit_chance.clamp(5, 100); // 5% minimum chance to hit, 100% max
    
    let is_miss = (rand::random::<u32>() % 100) >= hit_chance as u32;
    
    if is_miss {
        return DamageResult { damage: 0, is_crit: false, is_miss: true };
    }
    
    // Critical calculation
    // CRIT = LUK * 0.3
    let crit_chance = (luk as f32 * 0.3) as u32;
    let is_crit = (rand::random::<u32>() % 100) < crit_chance;
    
    if is_crit {
        // Critical ignores defense and maxes variance
        total_atk = base_atk + weapon_atk; 
    } else {
        // Defense calculation
        // Soft DEF = (VIT / 2) + (AGI / 5)
        let soft_def = (defender.vit as u32 / 2) + (defender.agi as u32 / 5);
        total_atk = total_atk.saturating_sub(soft_def);
    }
    
    DamageResult {
        damage: total_atk.max(1), // Minimum 1 damage
        is_crit,
        is_miss: false,
    }
}

pub fn calculate_skill_damage(
    attacker: &EntityStats,
    defender: &EntityStats,
    skill: &crate::database::skill::SkillModel,
    level: u16
) -> DamageResult {
    let lvl_idx = (level as usize).saturating_sub(1).min(9);
    let multiplier = skill.damage_multiplier[lvl_idx];

    let total_damage = match skill.skill_type {
        crate::database::skill::SkillType::Physical => {
            let base = calculate_melee_damage(attacker, defender);
            (base.damage as f32 * multiplier) as u32
        },
        crate::database::skill::SkillType::Magic => {
            // MATK = INT + (INT/7)^2
            let int = attacker.int as u32;
            let matk = int + (int / 7).pow(2);
            let raw_damage = (matk as f32 * multiplier) as u32;
            
            // MDEF = INT + (VIT/2)
            let mdef = defender.int as u32 + (defender.vit as u32 / 2);
            raw_damage.saturating_sub(mdef)
        },
        crate::database::skill::SkillType::Heal => {
            // Base Heal = [(BaseLevel + INT) / 8] * (HealLevel * 8 + 4)
            let base_heal = ((attacker.base_level as u32 + attacker.int as u32) / 8).max(1);
            let skill_mod = (level as u32 * 8) + 4;
            base_heal * skill_mod
        },
        crate::database::skill::SkillType::Support => {
            0
        }
    };

    DamageResult {
        damage: total_damage.max(1),
        is_crit: false,
        is_miss: false,
    }
}
