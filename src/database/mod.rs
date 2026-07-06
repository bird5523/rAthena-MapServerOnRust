pub mod item;
pub mod mob;
pub mod skill;

use sqlx::MySqlPool;

pub struct DatabaseManager {
    pub pool: MySqlPool,
    pub items: item::ItemDatabase,
    pub mobs: mob::MobDatabase,
    pub skills: skill::SkillDatabase,
}

impl DatabaseManager {
    pub async fn new(pool: MySqlPool) -> Self {
        let mut items = item::ItemDatabase::new();
        let mut mobs = mob::MobDatabase::new();
        
        // Use relative path to rAthena DB folder, try both depending on where the user ran the executable
        let mut db_path = "../rathena-master/db/re";
        if !std::path::Path::new(db_path).exists() {
            if std::path::Path::new("rathena-master/db/re").exists() {
                db_path = "rathena-master/db/re";
            }
        }
        
        if let Err(e) = items.load_all(db_path).await {
            tracing::error!("Could not load items: {:?}", e);
        }
        
        if let Err(e) = mobs.load_all(db_path).await {
            tracing::error!("Could not load mobs: {:?}", e);
        }

        Self {
            pool,
            items,
            mobs,
            skills: skill::SkillDatabase::new(),
        }
    }
}
