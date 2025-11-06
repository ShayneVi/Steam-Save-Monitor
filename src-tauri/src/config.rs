use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub ludusavi_path: String,
    pub backup_path: String,
    pub auto_start: bool,
    pub notifications_enabled: bool,
    pub game_executables: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steam_id_64: Option<String>,
    #[serde(default = "default_achievement_duration")]
    pub achievement_duration: u32,
}

fn default_achievement_duration() -> u32 {
    6
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ludusavi_path: String::new(),
            backup_path: String::new(),
            auto_start: true,
            notifications_enabled: true,
            game_executables: HashMap::new(),
            steam_api_key: None,
            steam_user_id: None,
            steam_id_64: None,
            achievement_duration: 6,
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
        // Handle auto-start registry changes if the setting changed
        #[cfg(target_os = "windows")]
        {
            if config.auto_start != self.config.auto_start {
                if config.auto_start {
                    let _ = Self::enable_auto_start();
                } else {
                    let _ = Self::disable_auto_start();
                }
            }
        }

        self.config = config;
        self.save_to_file().ok();
    }

    #[cfg(target_os = "windows")]
    fn enable_auto_start() -> Result<(), Box<dyn std::error::Error>> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_WRITE)?;

        // Get the current executable path
        let exe_path = std::env::current_exe()?;
        let exe_path_str = exe_path.to_string_lossy().to_string();

        // Set the registry value
        run_key.set_value("Steam Backup Manager", &exe_path_str)?;
        println!("Auto-start enabled: {}", exe_path_str);

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn disable_auto_start() -> Result<(), Box<dyn std::error::Error>> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_WRITE)?;

        // Delete the registry value
        run_key.delete_value("Steam Backup Manager")?;
        println!("Auto-start disabled");

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn enable_auto_start() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn disable_auto_start() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}