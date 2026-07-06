use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum SkillType {
    Magic,
    Physical,
    Heal,
    Support,
}

#[derive(Debug, Clone)]
pub struct SkillModel {
    pub id: u16,
    pub name: String,
    pub skill_type: SkillType,
    pub max_level: u16,
    pub sp_cost: Vec<u32>, // Index is level - 1
    pub cast_time: Vec<u32>, // In milliseconds. Index is level - 1
    pub cast_delay: Vec<u32>, // In milliseconds
    pub range: Vec<u16>,
    pub damage_multiplier: Vec<f32>, // E.g., 1.0 for 100% ATK
}

pub struct SkillDatabase {
    pub skills: HashMap<u16, SkillModel>,
}

impl SkillDatabase {
    pub fn new() -> Self {
        let mut db = Self {
            skills: HashMap::new(),
        };
        db.load_dummy_data();
        db
    }

    fn load_dummy_data(&mut self) {
        // 1. AL_HEAL (ID: 28)
        self.skills.insert(28, SkillModel {
            id: 28,
            name: "AL_HEAL".into(),
            skill_type: SkillType::Heal,
            max_level: 10,
            sp_cost: vec![13, 16, 19, 22, 25, 28, 31, 34, 37, 40],
            cast_time: vec![0; 10], // Instant cast
            cast_delay: vec![1000; 10], // 1s delay
            range: vec![9; 10],
            damage_multiplier: vec![0.0; 10], // Special formula for heal, not generic multiplier
        });

        // 2. SM_BASH (ID: 5)
        self.skills.insert(5, SkillModel {
            id: 5,
            name: "SM_BASH".into(),
            skill_type: SkillType::Physical,
            max_level: 10,
            sp_cost: vec![8, 8, 8, 8, 8, 15, 15, 15, 15, 15],
            cast_time: vec![0; 10],
            cast_delay: vec![1000; 10], // Assuming 1s animation/delay
            range: vec![1; 10], // Melee
            damage_multiplier: vec![1.3, 1.6, 1.9, 2.2, 2.5, 2.8, 3.1, 3.4, 3.7, 4.0], // 130% - 400%
        });

        // 3. MG_COLDBOLT / FIREBOLT (ID: 19 - Fire Bolt)
        self.skills.insert(19, SkillModel {
            id: 19,
            name: "MG_FIREBOLT".into(),
            skill_type: SkillType::Magic,
            max_level: 10,
            sp_cost: vec![12, 14, 16, 18, 20, 22, 24, 26, 28, 30],
            cast_time: vec![800, 1600, 2400, 3200, 4000, 4800, 5600, 6400, 7200, 8000], // 0.8s per level
            cast_delay: vec![800, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600],
            range: vec![9; 10],
            damage_multiplier: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0], // MATK * Level
        });
    }
}
