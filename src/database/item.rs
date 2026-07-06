use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{info, error};

#[derive(Debug, Clone, Deserialize)]
pub struct ItemModel {
    #[serde(rename = "Id")]
    pub id: i32,
    #[serde(rename = "AegisName", default)]
    pub aegis_name: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Type", default)]
    pub type_: String,
    #[serde(rename = "Buy", default)]
    pub price_buy: Option<i32>,
    #[serde(rename = "Sell", default)]
    pub price_sell: Option<i32>,
    #[serde(rename = "Weight", default)]
    pub weight: Option<i32>,
    // Add other fields as necessary based on the DB schema
}

#[derive(Deserialize)]
struct ItemDbFile {
    #[serde(rename = "Body")]
    body: Option<Vec<ItemModel>>,
}

pub struct ItemDatabase {
    pub items: HashMap<i32, ItemModel>,
    pub name_to_id: HashMap<String, i32>,
}

impl ItemDatabase {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            name_to_id: HashMap::new(),
        }
    }

    /// Loads items from YAML files in the `db/re` directory
    pub async fn load_all(&mut self, db_path: &str) -> anyhow::Result<()> {
        info!("Loading item database from YAML files in {}...", db_path);
        
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
                    // Only parse files that start with "item_db" (ignores item_combos, item_packages, etc.)
                    if name.starts_with("item_db") && name.ends_with(".yml") {
                        let content = fs::read_to_string(&file_path)?;
                        match serde_yml::from_str::<ItemDbFile>(&content) {
                            Ok(parsed) => {
                                if let Some(body) = parsed.body {
                                    for item in body {
                                        self.name_to_id.insert(item.aegis_name.clone(), item.id);
                                        self.items.insert(item.id, item);
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
            info!("No items found, injecting dummy Jellopy and Empty Bottle for testing.");
            let jellopy = ItemModel {
                id: 909,
                aegis_name: "Jellopy".into(),
                name: "Jellopy".into(),
                type_: "Etc".into(),
                price_buy: Some(6),
                price_sell: Some(3),
                weight: Some(10),
            };
            let empty_bottle = ItemModel {
                id: 512,
                aegis_name: "Empty_Bottle".into(),
                name: "Empty Bottle".into(),
                type_: "Etc".into(),
                price_buy: Some(2),
                price_sell: Some(1),
                weight: Some(20),
            };
            self.name_to_id.insert(jellopy.aegis_name.clone(), jellopy.id);
            self.items.insert(jellopy.id, jellopy);
            self.name_to_id.insert(empty_bottle.aegis_name.clone(), empty_bottle.id);
            self.items.insert(empty_bottle.id, empty_bottle);
            total_loaded += 2;
        }

        info!("Successfully loaded {} items from YAML database.", total_loaded);
        Ok(())
    }
}
