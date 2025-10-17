use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_found: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_backed_up: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LudusaviApiResponse {
    overall: OverallStats,
    games: HashMap<String, GameData>,
}

#[derive(Debug, Deserialize)]
struct OverallStats {
    #[allow(dead_code)]
    #[serde(rename = "totalGames")]
    total_games: i32,
    #[allow(dead_code)]
    #[serde(rename = "totalBytes")]
    total_bytes: i64,
}

#[derive(Debug, Deserialize)]
struct GameData {
    decision: String,
    files: Option<HashMap<String, FileData>>,
}

#[derive(Debug, Deserialize)]
struct FileData {
    bytes: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestCache {
    games: Vec<String>,
    timestamp: u64,
}

pub struct LudusaviManager {
    ludusavi_path: String,
    backup_path: String,
}

impl LudusaviManager {
    pub fn new(ludusavi_path: String, backup_path: String) -> Self {
        Self {
            ludusavi_path,
            backup_path,
        }
    }
    
    fn get_cache_path() -> PathBuf {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("steam-backup-manager");
        
        fs::create_dir_all(&cache_dir).ok();
        cache_dir.join("ludusavi_manifest_cache.json")
    }
    
    fn load_cache() -> Option<ManifestCache> {
        let cache_path = Self::get_cache_path();
        if let Ok(contents) = fs::read_to_string(&cache_path) {
            if let Ok(cache) = serde_json::from_str::<ManifestCache>(&contents) {
                // Cache is valid if it's less than 24 hours old
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                if now - cache.timestamp < 86400 {
                    return Some(cache);
                }
            }
        }
        None
    }
    
    fn save_cache(games: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        let cache = ManifestCache {
            games: games.to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };
        
        let cache_path = Self::get_cache_path();
        let json = serde_json::to_string(&cache)?;
        fs::write(&cache_path, json)?;
        Ok(())
    }
    
    fn clear_cache() -> Result<(), Box<dyn std::error::Error>> {
        let cache_path = Self::get_cache_path();
        if cache_path.exists() {
            fs::remove_file(&cache_path)?;
        }
        Ok(())
    }
    
    pub async fn test_connection(&self) -> Result<serde_json::Value, String> {
        if !Path::new(&self.ludusavi_path).exists() {
            return Ok(serde_json::json!({
                "success": false,
                "error": "Ludusavi executable not found at specified path"
            }));
        }
        
        match Command::new(&self.ludusavi_path)
            .arg("--version")
            .output()
        {
            Ok(output) => Ok(serde_json::json!({
                "success": output.status.success()
            })),
            Err(e) => Ok(serde_json::json!({
                "success": false,
                "error": e.to_string()
            })),
        }
    }
    
    pub async fn backup(&self, game_name: &str) -> Result<BackupResult, String> {
        let mut args = vec!["backup", "--api", "--force", game_name];
        
        if !self.backup_path.is_empty() {
            args.push("--path");
            args.push(&self.backup_path);
        }
        
        println!("Running Ludusavi: {:?} {:?}", self.ludusavi_path, args);
        
        match Command::new(&self.ludusavi_path)
            .args(&args)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW flag for Windows
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    println!("Ludusavi stderr: {}", error);
                    return Ok(BackupResult {
                        success: false,
                        not_found: None,
                        files_backed_up: None,
                        total_size: None,
                        error: Some(error),
                    });
                }
                
                let stdout = String::from_utf8_lossy(&output.stdout);
                println!("Ludusavi stdout: {}", stdout);
                
                let response: LudusaviApiResponse = serde_json::from_str(&stdout)
                    .map_err(|e| format!("Failed to parse response: {}", e))?;
                
                if let Some(game_data) = response.games.get(game_name) {
                    if game_data.decision == "Ignored" {
                        return Ok(BackupResult {
                            success: false,
                            not_found: Some(true),
                            files_backed_up: None,
                            total_size: None,
                            error: None,
                        });
                    }
                    
                    let file_count = game_data.files.as_ref().map(|f| f.len()).unwrap_or(0);
                    let total_bytes: i64 = game_data.files
                        .as_ref()
                        .map(|files| files.values().map(|f| f.bytes).sum())
                        .unwrap_or(0);
                    
                    Ok(BackupResult {
                        success: true,
                        not_found: None,
                        files_backed_up: Some(file_count),
                        total_size: Some(Self::format_bytes(total_bytes)),
                        error: None,
                    })
                } else {
                    Ok(BackupResult {
                        success: false,
                        not_found: Some(true),
                        files_backed_up: None,
                        total_size: None,
                        error: None,
                    })
                }
            }
            Err(e) => Ok(BackupResult {
                success: false,
                not_found: None,
                files_backed_up: None,
                total_size: None,
                error: Some(e.to_string()),
            }),
        }
    }
    
    pub async fn get_manifest_games(&self) -> Result<Vec<String>, String> {
        // Try to load from cache first
        if let Some(cache) = Self::load_cache() {
            println!("Using cached manifest with {} games", cache.games.len());
            return Ok(cache.games);
        }

        if !Path::new(&self.ludusavi_path).exists() {
            return Err("Ludusavi executable not found at specified path".to_string());
        }
        
        println!("Loading manifest from Ludusavi (this may take a moment)...");
        let output = Command::new(&self.ludusavi_path)
            .args(&["manifest", "show", "--api"])
            .output()
            .map_err(|e| e.to_string())?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to get manifest: {}", error));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let manifest: HashMap<String, serde_json::Value> = serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse manifest: {}", e))?;
        
        let mut games: Vec<String> = manifest.keys().cloned().collect();
        games.sort();
        
        // Save to cache
        let _ = Self::save_cache(&games);
        
        Ok(games)
    }
    
    pub fn extract_exe_name(path: &str) -> String {
        Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase()
    }
    
    pub async fn clear_manifest_cache() -> Result<(), String> {
        Self::clear_cache().map_err(|e| e.to_string())
    }
    
    fn format_bytes(bytes: i64) -> String {
        if bytes == 0 {
            return "0 Bytes".to_string();
        }
        
        let k = 1024_f64;
        let sizes = ["Bytes", "KB", "MB", "GB"];
        let i = (bytes as f64).log(k).floor() as usize;
        let size = (bytes as f64) / k.powi(i as i32);
        
        format!("{:.2} {}", size, sizes[i.min(sizes.len() - 1)])
    }
}