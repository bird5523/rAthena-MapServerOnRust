use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{info, error};

#[derive(Debug, Clone, Deserialize)]
pub struct MobDrop {
    #[serde(rename = "Item")]
    pub item: String, // Aegis name of the item
    #[serde(rename = "Rate")]
    pub rate: i32, // Rate out of 10000 (100% = 10000)
}

#[derive(Debug, Clone, Deserialize)]
pub struct MobModel {
    #[serde(rename = "Id")]
    pub id: i32,
    #[serde(rename = "AegisName", default)]
    pub aegis_name: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Level", default)]
    pub level: i32,
    #[serde(rename = "Hp", default)]
    pub hp: i32,
    #[serde(rename = "BaseExp", default)]
    pub base_exp: i32,
    #[serde(rename = "JobExp", default)]
    pub job_exp: i32,
    #[serde(rename = "Attack", default)]
    pub attack: i32,
    #[serde(rename = "Attack2", default)]
    pub attack2: i32,
    #[serde(rename = "Defense", default)]
    pub defense: i32,
    #[serde(rename = "Drops", default)]
    pub drops: Vec<MobDrop>,
}

#[derive(Deserialize)]
struct MobDbFile {
    #[serde(rename = "Body")]
    body: Option<Vec<MobModel>>,
}

pub struct MobDatabase {
    pub mobs: HashMap<i32, MobModel>,
}

impl MobDatabase {
    pub fn new() -> Self {
        Self {
            mobs: HashMap::new(),
        }
    }

    /// Loads mobs from `mob_db.yml` file
    pub async fn load_all(&mut self, db_path: &str) -> anyhow::Result<()> {
        info!("Loading mob database from YAML files in {}...", db_path);
        
        let path = Path::new(db_path);
        if !path.exists() {
            error!("Database path does not exist: {}", db_path);
            return Ok(());
        }

        let mut total_loaded = 0;

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            
            if file_path.is_file() {
                if let Some(name) = file_path.file_name().and_then(|n| n.to_str()) {
                    // Filter mob database files
                    if name.starts_with("mob_db") && name.ends_with(".yml") {
                        let content = fs::read_to_string(&file_path)?;
                        match serde_yml::from_str::<MobDbFile>(&content) {
                            Ok(parsed) => {
                                if let Some(body) = parsed.body {
                                    for mob in body {
                                        self.mobs.insert(mob.id, mob);
                                        total_loaded += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse {}: {}", name, e);
                            }
                        }
                    }
                }
            }
        }
        
        if total_loaded == 0 {
            info!("No mobs found, injecting dummy Poring for testing.");
            let poring = MobModel {
                id: 1002,
                aegis_name: "PORING".into(),
                name: "Poring".into(),
                level: 1,
                hp: 50,
                base_exp: 2,
                job_exp: 1,
                attack: 7,
                attack2: 10,
                defense: 0,
                drops: vec![
                    MobDrop { item: "Jellopy".into(), rate: 10000 },      // 100% Jellopy
                    MobDrop { item: "Empty_Bottle".into(), rate: 5000 },  // 50% Empty Bottle
                ],
            };
            self.mobs.insert(poring.id, poring);
            total_loaded += 1;
        }

        info!("Successfully loaded {} mobs from YAML database.", total_loaded);
        Ok(())
    }
}
