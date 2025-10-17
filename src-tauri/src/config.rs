use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub steam_api_key: String,
    pub steam_user_id: String,
    pub ludusavi_path: String,
    pub backup_path: String,
    pub auto_start: bool,
    pub notifications_enabled: bool,
    pub game_executables: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            steam_api_key: String::new(),
            steam_user_id: String::new(),
            ludusavi_path: String::new(),
            backup_path: String::new(),
            auto_start: true,
            notifications_enabled: true,
            game_executables: HashMap::new(),
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
    config: AppConfig,
}

impl ConfigManager {
    pub fn new() -> Self {
        let config_path = Self::get_config_path();
        let config = Self::load_from_file(&config_path);
        
        Self { config_path, config }
    }
    
    fn get_config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .expect("Could not find config directory")
            .join("steam-backup-manager");
        
        fs::create_dir_all(&config_dir).ok();
        config_dir.join("config.json")
    }
    
    fn load_from_file(path: &PathBuf) -> AppConfig {
        if let Ok(contents) = fs::read_to_string(path) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            AppConfig::default()
        }
    }
    
    fn save_to_file(&self) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.config_path, json)?;
        Ok(())
    }
    
    pub fn get_all(&self) -> AppConfig {
        self.config.clone()
    }
    
    pub fn set_all(&mut self, config: AppConfig) {
        self.config = config;
        self.save_to_file().ok();
    }
}