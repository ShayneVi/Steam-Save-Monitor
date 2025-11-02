// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod steam_monitor;
mod process_monitor;
mod ludusavi;
mod notifications;
mod achievements;
mod achievement_scanner;
mod steam_achievements;
mod achievement_watcher;
mod overlay;

use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayEvent, Manager, State, Window};
use tauri::api::dialog;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use std::sync::mpsc::{channel, Sender};

use config::{ConfigManager, AppConfig};
use steam_monitor::SteamMonitor;
use process_monitor::ProcessMonitor;
use ludusavi::LudusaviManager;
use notifications::NotificationManager;
use achievements::{AchievementDatabase, GameAchievementSummary, Achievement};
use steam_achievements::{SteamAchievementClient, SteamGameSearchResult};
use achievement_watcher::{AchievementWatcher, AchievementUnlockEvent};
use overlay::OverlayManager;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<ConfigManager>>,
    steam_handle: Arc<Mutex<Option<mpsc::Sender<MonitorCommand>>>>,
    process_handle: Arc<Mutex<Option<mpsc::Sender<bool>>>>,
    notification_manager: Arc<Mutex<NotificationManager>>,
    achievement_db_path: Arc<Mutex<Option<PathBuf>>>,
    achievement_watcher: Arc<Mutex<Option<Arc<AchievementWatcher>>>>,
    overlay_manager: Arc<Mutex<OverlayManager>>,
    achievement_duration: Arc<Mutex<u32>>, // Duration in seconds
}

enum MonitorCommand {
    Stop,
    Pause,
    Resume,
}

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().unwrap();
    Ok(config.get_all())
}

#[tauri::command]
async fn save_config(
    config: AppConfig,
    state: State<'_, AppState>,
    window: Window,
) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.set_all(config.clone());
    }
    
    // Restart monitors
    stop_monitors(&state).await;
    start_monitors(&state, window).await;
    
    Ok(())
}

#[tauri::command]
async fn browse_file() -> Result<Option<String>, String> {
    let path = dialog::blocking::FileDialogBuilder::new()
        .add_filter("All Files", &["*"])
        .add_filter("Executables", &["exe"])
        .add_filter("Audio", &["mp3", "wav", "ogg", "flac", "aac"])
        .add_filter("Fonts", &["ttf", "otf", "woff", "woff2"])
        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "svg", "ico"])
        .pick_file();

    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
async fn browse_folder() -> Result<Option<String>, String> {
    let path = dialog::blocking::FileDialogBuilder::new()
        .pick_folder();
    
    Ok(path.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
async fn test_ludusavi(path: String) -> Result<serde_json::Value, String> {
    let manager = LudusaviManager::new(path, String::new());
    manager.test_connection().await
}

#[tauri::command]
async fn get_ludusavi_manifest(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let (ludusavi_path, backup_path) = {
        let config = state.config.lock().unwrap();
        let cfg = config.get_all();

        if cfg.ludusavi_path.is_empty() {
            return Err("Ludusavi path not configured".to_string());
        }

        (cfg.ludusavi_path, cfg.backup_path)
    };

    let manager = LudusaviManager::new(ludusavi_path, backup_path);
    manager.get_manifest_games().await
}

#[tauri::command]
async fn get_all_achievements(state: State<'_, AppState>) -> Result<Vec<GameAchievementSummary>, String> {
    // Open database connection
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => db.get_all_games(),
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn get_game_achievements(app_id: u32, state: State<'_, AppState>) -> Result<Vec<Achievement>, String> {
    // Open database connection
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => db.get_game_achievements(app_id),
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn update_achievement_status(
    achievement_id: i64,
    achieved: bool,
    unlock_time: Option<i64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Open database connection
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => db.update_achievement_status(achievement_id, achieved, unlock_time),
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn sync_achievements(state: State<'_, AppState>) -> Result<String, String> {
    println!("Starting achievement synchronization...");

    // Get API key, user ID, and Steam64 ID from config
    let (api_key, steam_user_id, steam_id_64) = {
        let config = state.config.lock().unwrap();
        let cfg = config.get_all();
        (cfg.steam_api_key, cfg.steam_user_id, cfg.steam_id_64)
    };

    // Initialize local achievement scanner (for librarycache)
    let steam_path = PathBuf::from(r"C:\Program Files (x86)\Steam");
    let local_scanner = achievement_scanner::AchievementScanner::new(steam_path, steam_user_id.clone()).ok();

    // Initialize Steam achievement client (for API)
    let steam_client = SteamAchievementClient::new(api_key, steam_id_64.clone())
        .map_err(|e| format!("Failed to initialize Steam client: {}", e))?;

    // Get database path for opening connections as needed
    let db_path = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        path_guard.clone()
    };

    let db_path = match db_path {
        Some(path) => path,
        None => return Err("Achievement database not initialized".to_string()),
    };

    // Get all installed Steam games
    let library_folders = get_steam_library_folders()?;
    let mut total_achievements = 0;
    let mut games_scanned = 0;

    for library_path in library_folders {
        let steamapps_path = library_path.join("steamapps");
        if !steamapps_path.exists() {
            continue;
        }

        // Read all appmanifest files
        if let Ok(entries) = std::fs::read_dir(&steamapps_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(filename) = path.file_name() {
                    let filename_str = filename.to_string_lossy();
                    if filename_str.starts_with("appmanifest_") && filename_str.ends_with(".acf") {
                        if let Some((app_id, game_name)) = parse_appmanifest_basic(&path) {
                            println!("Scanning achievements for: {} ({})", game_name, app_id);

                            // PHASE 1: Scan all sources and collect results
                            let mut source_results: Vec<(&str, usize)> = Vec::new();

                            // PRIORITY 1: Try Online-fix
                            if let Some(ref scanner) = local_scanner {
                                match scanner.scan_onlinefix_achievements(app_id, &game_name, db_path.clone(), &steam_client).await {
                                    Ok(count) => {
                                        println!("  ℹ Online-fix: {} unlocked achievements", count);
                                        source_results.push(("Online-fix", count));
                                    }
                                    Err(e) => {
                                        if !e.contains("No achievements found") && !e.contains("does not exist") {
                                            println!("  ⚠ Online-fix scan error: {}", e);
                                        }
                                    }
                                }
                            }

                            // PRIORITY 2: Try Steamtools (librarycache)
                            if let Some(ref scanner) = local_scanner {
                                match scanner.scan_steam_achievements(app_id, &game_name, db_path.clone(), &steam_client).await {
                                    Ok(count) => {
                                        println!("  ℹ Steamtools: {} unlocked achievements", count);
                                        source_results.push(("Steamtools", count));
                                    }
                                    Err(e) => {
                                        println!("  ⚠ Steamtools scan error: {}", e);
                                    }
                                }
                            }

                            // PRIORITY 3: Try Goldberg
                            if let Some(ref scanner) = local_scanner {
                                match scanner.scan_goldberg_achievements(app_id, &game_name, db_path.clone(), &steam_client).await {
                                    Ok(count) => {
                                        println!("  ℹ Goldberg: {} unlocked achievements", count);
                                        source_results.push(("Goldberg", count));
                                    }
                                    Err(_) => {}
                                }
                            }

                            // PRIORITY 4: Try Steam API
                            let achievements_result = steam_client.scan_achievements_for_game(app_id, &game_name).await;
                            match achievements_result {
                                Ok(achievements) if !achievements.is_empty() => {
                                    if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                                        for ach in &achievements {
                                            let _ = db.insert_or_update_achievement(ach);
                                        }
                                        let unlocked = achievements.iter().filter(|a| a.achieved).count();
                                        println!("  ℹ Steam Web API: {} unlocked achievements", unlocked);
                                        source_results.push(("Steam Web API", unlocked));
                                    }
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    if !e.contains("No achievements found") {
                                        println!("  ⚠ Error scanning {}: {}", game_name, e);
                                    }
                                }
                            }

                            // PHASE 2: Choose the best source if we found any
                            if !source_results.is_empty() {
                                let best_source = source_results.iter().max_by_key(|(_, count)| count).unwrap();
                                println!("  ✓ Choosing {} with {} unlocked achievements", best_source.0, best_source.1);

                                // PHASE 3: Delete all achievements for this game
                                if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                                    let _ = db.delete_game_achievements(app_id);
                                }

                                // PHASE 4: Rescan only the winning source
                                match best_source.0 {
                                    "Online-fix" => {
                                        if let Some(ref scanner) = local_scanner {
                                            let _ = scanner.scan_onlinefix_achievements(app_id, &game_name, db_path.clone(), &steam_client).await;
                                        }
                                    }
                                    "Steamtools" => {
                                        if let Some(ref scanner) = local_scanner {
                                            let _ = scanner.scan_steam_achievements(app_id, &game_name, db_path.clone(), &steam_client).await;
                                        }
                                    }
                                    "Goldberg" => {
                                        if let Some(ref scanner) = local_scanner {
                                            let _ = scanner.scan_goldberg_achievements(app_id, &game_name, db_path.clone(), &steam_client).await;
                                        }
                                    }
                                    "Steam Web API" => {
                                        // Rescan and insert
                                        if let Ok(achievements) = steam_client.scan_achievements_for_game(app_id, &game_name).await {
                                            if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                                                for ach in &achievements {
                                                    let _ = db.insert_or_update_achievement(ach);
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }

                                total_achievements += best_source.1;
                                games_scanned += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(format!("Scanned {} games, found {} achievements", games_scanned, total_achievements))
}

#[tauri::command]
async fn add_manual_achievement(
    app_id: u32,
    game_name: String,
    achievement_id: String,
    display_name: String,
    description: String,
    achieved: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Open database connection
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => {
            let achievement = Achievement {
                id: None,
                app_id,
                game_name,
                achievement_id,
                display_name,
                description,
                icon_url: None,
                icon_gray_url: None,
                hidden: false,
                achieved,
                unlock_time: if achieved {
                    Some(chrono::Utc::now().timestamp())
                } else {
                    None
                },
                source: "Manual".to_string(),
                last_updated: chrono::Utc::now().timestamp(),
                global_unlock_percentage: None,
            };

            db.insert_or_update_achievement(&achievement)
        }
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn export_achievements(state: State<'_, AppState>) -> Result<String, String> {
    // Open database connection
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => db.export_to_json(),
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn export_game_achievements(app_id: u32, game_name: String, state: State<'_, AppState>) -> Result<String, String> {
    use std::fs;
    use std::io::Write;

    // Get database
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    let db = match db {
        Some(db) => db,
        None => return Err("Achievement database not initialized".to_string()),
    };

    // Get all achievements for this game
    let all_achievements = db.get_game_achievements(app_id)?;

    // Filter only unlocked achievements
    let unlocked: Vec<_> = all_achievements.iter()
        .filter(|a| a.achieved)
        .collect();

    // Save count before consuming iterator
    let unlocked_count = unlocked.len();

    // Convert to Steam API format
    // Format: {"<achievement_id>": {"UnlockTime": <timestamp>}}
    let mut steam_format = serde_json::Map::new();
    for achievement in unlocked {
        let mut achievement_data = serde_json::Map::new();
        achievement_data.insert(
            "UnlockTime".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from(achievement.unlock_time.unwrap_or(0))
            )
        );
        steam_format.insert(
            achievement.achievement_id.clone(),
            serde_json::Value::Object(achievement_data)
        );
    }

    let json_string = serde_json::to_string_pretty(&steam_format)
        .map_err(|e| format!("Failed to serialize to JSON: {}", e))?;

    // Get Documents folder
    let documents_dir = match dirs::document_dir() {
        Some(dir) => dir,
        None => return Err("Could not find Documents folder".to_string()),
    };

    // Create Steam Backup Monitor folder
    let export_dir = documents_dir.join("Steam Backup Monitor");
    if !export_dir.exists() {
        fs::create_dir_all(&export_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Sanitize game name for filename
    let safe_game_name: String = game_name.chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c
        })
        .collect();

    // Create file path
    let file_path = export_dir.join(format!("{}.json", safe_game_name));

    // Write to file (overwrites if exists)
    let mut file = fs::File::create(&file_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

    file.write_all(json_string.as_bytes())
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(format!("Exported {} unlocked achievements to: {}", unlocked_count, file_path.display()))
}

#[tauri::command]
async fn search_steam_games(query: String, state: State<'_, AppState>) -> Result<Vec<SteamGameSearchResult>, String> {
    let (api_key, steam_id_64) = {
        let config = state.config.lock().unwrap();
        let cfg = config.get_all();
        (cfg.steam_api_key, cfg.steam_id_64)
    };

    let steam_client = SteamAchievementClient::new(api_key, steam_id_64)
        .map_err(|e| format!("Failed to initialize Steam client: {}", e))?;

    steam_client.search_games(&query).await
}

#[derive(Clone, Serialize, Deserialize)]
struct SourceOption {
    name: String,
    unlocked_count: usize,
    total_count: usize,
}

#[tauri::command]
async fn check_game_sources(
    app_id: u32,
    game_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<SourceOption>, String> {
    println!("Checking sources for {} (app_id: {})...", game_name, app_id);

    // Get API key, user ID, and Steam64 ID from config
    let (api_key, steam_user_id, steam_id_64) = {
        let config = state.config.lock().unwrap();
        let cfg = config.get_all();
        (cfg.steam_api_key, cfg.steam_user_id, cfg.steam_id_64)
    };

    // Get database path
    let db_path = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        path_guard.clone()
    };

    let db_path = match db_path {
        Some(path) => path,
        None => return Err("Achievement database not initialized".to_string()),
    };

    // Create Steam API client
    let steam_client = SteamAchievementClient::new(api_key.clone(), steam_id_64.clone())
        .map_err(|e| format!("Failed to initialize Steam client: {}", e))?;

    let steam_path = PathBuf::from(r"C:\Program Files (x86)\Steam");

    // Scan all sources and collect results
    let mut source_options: Vec<SourceOption> = Vec::new();

    // PRIORITY 1: Try Online-fix
    if let Ok(scanner) = achievement_scanner::AchievementScanner::new(steam_path.clone(), steam_user_id.clone()) {
        match scanner.scan_onlinefix_achievements(app_id, &game_name, db_path.clone(), &steam_client).await {
            Ok(count) => {
                // Get total count from database
                if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                    if let Ok(achievements) = db.get_game_achievements(app_id) {
                        let total = achievements.len();
                        println!("  ✓ Online-fix: {} unlocked / {} total", count, total);
                        source_options.push(SourceOption {
                            name: "Online-fix".to_string(),
                            unlocked_count: count,
                            total_count: total,
                        });
                    }
                }
                // Clear the database after checking
                if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                    let _ = db.delete_game_achievements(app_id);
                }
            }
            Err(e) => {
                if !e.contains("No achievements found") && !e.contains("does not exist") {
                    println!("  ⚠ Online-fix scan error: {}", e);
                }
            }
        }
    }

    // PRIORITY 2: Try Steamtools (librarycache)
    if let Ok(scanner) = achievement_scanner::AchievementScanner::new(steam_path.clone(), steam_user_id.clone()) {
        match scanner.scan_steam_achievements(app_id, &game_name, db_path.clone(), &steam_client).await {
            Ok(count) => {
                // Get total count from database
                if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                    if let Ok(achievements) = db.get_game_achievements(app_id) {
                        let total = achievements.len();
                        println!("  ✓ Steamtools: {} unlocked / {} total", count, total);
                        source_options.push(SourceOption {
                            name: "Steamtools".to_string(),
                            unlocked_count: count,
                            total_count: total,
                        });
                    }
                }
                // Clear the database after checking
                if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                    let _ = db.delete_game_achievements(app_id);
                }
            }
            Err(e) => {
                println!("  ⚠ Steamtools scan error: {}", e);
            }
        }
    }

    // PRIORITY 3: Try Goldberg emulator achievements
    if let Ok(scanner) = achievement_scanner::AchievementScanner::new(steam_path.clone(), steam_user_id.clone()) {
        match scanner.scan_goldberg_achievements(app_id, &game_name, db_path.clone(), &steam_client).await {
            Ok(count) => {
                // Get total count from database
                if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                    if let Ok(achievements) = db.get_game_achievements(app_id) {
                        let total = achievements.len();
                        println!("  ✓ Goldberg: {} unlocked / {} total", count, total);
                        source_options.push(SourceOption {
                            name: "Goldberg".to_string(),
                            unlocked_count: count,
                            total_count: total,
                        });
                    }
                }
                // Clear the database after checking
                if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                    let _ = db.delete_game_achievements(app_id);
                }
            }
            Err(_) => {
                // Game not found in this source
            }
        }
    }

    // PRIORITY 4: Try Steam Web API
    println!("  Fetching from Steam Web API...");
    match steam_client.scan_achievements_for_game(app_id, &game_name).await {
        Ok(achievements) if !achievements.is_empty() => {
            if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                for ach in &achievements {
                    let _ = db.insert_or_update_achievement(ach);
                }
                let unlocked = achievements.iter().filter(|a| a.achieved).count();
                let total = achievements.len();
                println!("  ✓ Steam Web API: {} unlocked / {} total", unlocked, total);
                source_options.push(SourceOption {
                    name: "Steam Web API".to_string(),
                    unlocked_count: unlocked,
                    total_count: total,
                });
                // Clear the database after checking
                let _ = db.delete_game_achievements(app_id);
            }
        }
        Ok(_) => {}
        Err(e) => {
            if !e.contains("No achievements found") {
                println!("  ⚠ Steam API error: {}", e);
            }
        }
    }

    // No achievements found anywhere
    if source_options.is_empty() {
        return Err("No achievements found for this game in any source".to_string());
    }

    Ok(source_options)
}

#[tauri::command]
async fn add_game_from_source(
    app_id: u32,
    game_name: String,
    source: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    println!("Adding {} (app_id: {}) from {}...", game_name, app_id, source);

    // Get API key, user ID, and Steam64 ID from config
    let (api_key, steam_user_id, steam_id_64) = {
        let config = state.config.lock().unwrap();
        let cfg = config.get_all();
        (cfg.steam_api_key, cfg.steam_user_id, cfg.steam_id_64)
    };

    // Get database path
    let db_path = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        path_guard.clone()
    };

    let db_path = match db_path {
        Some(path) => path,
        None => return Err("Achievement database not initialized".to_string()),
    };

    // Create Steam API client
    let steam_client = SteamAchievementClient::new(api_key.clone(), steam_id_64.clone())
        .map_err(|e| format!("Failed to initialize Steam client: {}", e))?;

    let steam_path = PathBuf::from(r"C:\Program Files (x86)\Steam");

    // Delete any existing achievements for this game
    if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
        let _ = db.delete_game_achievements(app_id);
    }

    // Scan from the selected source
    let unlocked_count = match source.as_str() {
        "Online-fix" => {
            if let Ok(scanner) = achievement_scanner::AchievementScanner::new(steam_path.clone(), steam_user_id.clone()) {
                scanner.scan_onlinefix_achievements(app_id, &game_name, db_path.clone(), &steam_client).await?
            } else {
                return Err("Failed to initialize scanner".to_string());
            }
        }
        "Steamtools" => {
            if let Ok(scanner) = achievement_scanner::AchievementScanner::new(steam_path.clone(), steam_user_id.clone()) {
                scanner.scan_steam_achievements(app_id, &game_name, db_path.clone(), &steam_client).await?
            } else {
                return Err("Failed to initialize scanner".to_string());
            }
        }
        "Goldberg" => {
            if let Ok(scanner) = achievement_scanner::AchievementScanner::new(steam_path.clone(), steam_user_id.clone()) {
                scanner.scan_goldberg_achievements(app_id, &game_name, db_path.clone(), &steam_client).await?
            } else {
                return Err("Failed to initialize scanner".to_string());
            }
        }
        "Steam Web API" => {
            match steam_client.scan_achievements_for_game(app_id, &game_name).await {
                Ok(achievements) => {
                    if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                        for ach in &achievements {
                            db.insert_or_update_achievement(ach)?;
                        }
                        achievements.iter().filter(|a| a.achieved).count()
                    } else {
                        return Err("Failed to open database".to_string());
                    }
                }
                Err(e) => return Err(format!("Failed to scan Steam API: {}", e)),
            }
        }
        _ => return Err(format!("Unknown source: {}", source)),
    };

    Ok(format!("Added {} with {} unlocked achievements (from {})", game_name, unlocked_count, source))
}

#[tauri::command]
async fn remove_game_from_tracking(
    app_id: u32,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Open database connection
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => {
            db.delete_game_achievements(app_id)?;
            Ok(format!("Removed game (app_id: {}) and all its achievements", app_id))
        }
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn get_all_exclusions(state: State<'_, AppState>) -> Result<Vec<achievements::Exclusion>, String> {
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => db.get_all_exclusions(),
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn add_exclusion(
    app_id: u32,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => {
            db.add_exclusion(app_id, name)?;
            // No need to restart monitors - they check exclusions dynamically on each scan
            println!("Added app_id {} to exclusions", app_id);
            Ok(())
        }
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn remove_exclusion(
    app_id: u32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        match &*path_guard {
            Some(path) => AchievementDatabase::new(path.clone()).ok(),
            None => None,
        }
    };

    match db {
        Some(db) => {
            db.remove_exclusion(app_id)?;
            // No need to restart monitors - they check exclusions dynamically on each scan
            println!("Removed app_id {} from exclusions", app_id);
            Ok(())
        }
        None => Err("Achievement database not initialized".to_string()),
    }
}

#[tauri::command]
async fn fetch_achievement_icon(url: String) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose};
    use std::time::Duration;

    // Create HTTP client with longer timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // Fetch the image from Steam CDN with retries
    let mut last_error = String::new();
    for attempt in 1..=3 {
        match client.get(&url).send().await {
            Ok(response) => {
                // Get the image bytes
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| format!("Failed to read icon bytes: {}", e))?;

                // Convert to base64
                let base64 = general_purpose::STANDARD.encode(&bytes);

                // Determine MIME type from URL extension
                let mime_type = if url.ends_with(".jpg") || url.ends_with(".jpeg") {
                    "image/jpeg"
                } else if url.ends_with(".png") {
                    "image/png"
                } else {
                    "image/jpeg" // default
                };

                // Return as data URL
                return Ok(format!("data:{};base64,{}", mime_type, base64));
            }
            Err(e) => {
                last_error = format!("Attempt {}/3 failed: {}", attempt, e);
                if attempt < 3 {
                    // Wait before retrying
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    Err(format!("Failed to fetch icon after 3 attempts: {}", last_error))
}

#[tauri::command]
fn play_windows_notification_sound() -> Result<(), String> {
    use windows::Win32::Media::Audio::{PlaySoundA, SND_ALIAS, SND_ASYNC};
    use windows::core::PCSTR;
    use std::ffi::CString;

    std::thread::spawn(move || {
        unsafe {
            let sound_alias = CString::new("SystemNotification").unwrap_or_default();
            let _ = PlaySoundA(
                PCSTR(sound_alias.as_ptr() as *const u8),
                None,
                SND_ALIAS | SND_ASYNC,
            );
        }
    });

    Ok(())
}

#[tauri::command]
fn debug_log(message: String) {
    println!("[OVERLAY DEBUG] {}", message);
}

#[tauri::command]
fn check_backup_exists(game_name: String) -> Result<Option<String>, String> {
    // Get Documents folder
    let documents_dir = match dirs::document_dir() {
        Some(dir) => dir,
        None => return Err("Could not find Documents folder".to_string()),
    };

    // Check Steam Backup Monitor folder
    let export_dir = documents_dir.join("Steam Backup Monitor");
    if !export_dir.exists() {
        return Ok(None);
    }

    // Sanitize game name for filename
    let safe_game_name: String = game_name.chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c
        })
        .collect();

    // Check if backup file exists
    let file_path = export_dir.join(format!("{}.json", safe_game_name));
    if file_path.exists() {
        Ok(Some(file_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn restore_from_backup(
    app_id: u32,
    game_name: String,
    backup_path: String,
    state: State<'_, AppState>
) -> Result<usize, String> {
    use std::fs;

    // Read backup file
    let backup_content = fs::read_to_string(&backup_path)
        .map_err(|e| format!("Failed to read backup file: {}", e))?;

    // Parse JSON (Steam API format: {"achievement_id": {"UnlockTime": timestamp}})
    let backup_data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&backup_content)
        .map_err(|e| format!("Failed to parse backup file: {}", e))?;

    // Get database
    let db_path = {
        let path_guard = state.achievement_db_path.lock().unwrap();
        path_guard.clone()
    };

    let db_path = match db_path {
        Some(path) => path,
        None => return Err("Achievement database not initialized".to_string()),
    };

    let db = AchievementDatabase::new(db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Get all achievements for this game (they should already be in DB from the source scan)
    let all_achievements = db.get_game_achievements(app_id)?;

    let mut restored_count = 0;

    // Update achievements that are in the backup
    for achievement in all_achievements {
        if let Some(backup_entry) = backup_data.get(&achievement.achievement_id) {
            if let Some(unlock_time_value) = backup_entry.get("UnlockTime") {
                if let Some(unlock_time) = unlock_time_value.as_i64() {
                    // Update achievement status to unlocked with the backup timestamp
                    if let Some(id) = achievement.id {
                        db.update_achievement_status(id, true, Some(unlock_time))
                            .map_err(|e| format!("Failed to update achievement: {}", e))?;
                        restored_count += 1;
                    }
                }
            }
        }
    }

    Ok(restored_count)
}

#[tauri::command]
fn read_audio_file(file_path: String) -> Result<Vec<u8>, String> {
    use std::fs;

    println!("[OVERLAY DEBUG] Reading audio file: {}", file_path);

    match fs::read(&file_path) {
        Ok(bytes) => {
            println!("[OVERLAY DEBUG] Successfully read {} bytes", bytes.len());
            Ok(bytes)
        }
        Err(e) => {
            let error_msg = format!("Failed to read audio file: {}", e);
            println!("[OVERLAY DEBUG] {}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
async fn test_overlay(state: State<'_, AppState>) -> Result<(), String> {
    // Use NotificationManager to show achievement on overlay
    state.notification_manager.lock().unwrap().show_achievement_unlock(
        "Test Game",
        "First Steps",
        "Complete the tutorial",
        Some("https://cdn.cloudflare.steamstatic.com/steamcommunity/public/images/apps/default_icon.jpg"),
        Some(85.0) // Uncommon rarity for testing
    );

    Ok(())
}

#[tauri::command]
async fn get_achievement_duration(state: State<'_, AppState>) -> Result<u32, String> {
    let duration = *state.achievement_duration.lock().unwrap();
    Ok(duration)
}

#[tauri::command]
async fn set_achievement_duration(duration: u32, state: State<'_, AppState>) -> Result<(), String> {
    *state.achievement_duration.lock().unwrap() = duration;
    println!("[Backend] Achievement duration set to {} seconds", duration);
    Ok(())
}

#[tauri::command]
async fn sync_settings_to_overlay(achievement_settings: serde_json::Value, rarity_settings: serde_json::Value, app: tauri::AppHandle) -> Result<(), String> {
    // Emit settings to ALL windows (including overlay)
    app.emit_all("achievement-settings-sync", &achievement_settings)
        .map_err(|e| format!("Failed to emit achievement settings: {}", e))?;

    app.emit_all("rarity-settings-sync", &rarity_settings)
        .map_err(|e| format!("Failed to emit rarity settings: {}", e))?;

    println!("[Backend] Settings synced to all windows");
    Ok(())
}

#[tauri::command]
async fn test_rarity_notification(rarity: String, state: State<'_, AppState>) -> Result<(), String> {
    // Map rarity percentage for testing
    let (name, description, percentage) = match rarity.as_str() {
        "Common" => ("Common Achievement", "30%+ of players have this", 35.0),
        "Uncommon" => ("Uncommon Achievement", "20-29% of players have this", 25.0),
        "Rare" => ("Rare Achievement", "13-19% of players have this", 15.0),
        "Ultra Rare" => ("Ultra Rare Achievement", "5-12% of players have this", 8.0),
        "Legendary" => ("Legendary Achievement", "0-4% of players have this", 2.0),
        _ => ("Test Achievement", "Unknown rarity", 50.0),
    };

    // Use NotificationManager to show achievement on overlay with rarity percentage
    state.notification_manager.lock().unwrap().show_achievement_unlock(
        "Test Game",
        name,
        description,
        Some("https://cdn.cloudflare.steamstatic.com/steamcommunity/public/images/apps/default_icon.jpg"),
        Some(percentage)
    );

    Ok(())
}

// Helper functions
fn get_steam_library_folders() -> Result<Vec<PathBuf>, String> {
    let steam_path = PathBuf::from(r"C:\Program Files (x86)\Steam");
    let mut folders = vec![steam_path.clone()];

    let libraryfolders_path = steam_path.join("steamapps").join("libraryfolders.vdf");
    if let Ok(contents) = std::fs::read_to_string(&libraryfolders_path) {
        if let Ok(re) = regex::Regex::new(r#""path"\s+"([^"]+)""#) {
            for cap in re.captures_iter(&contents) {
                if let Some(path_match) = cap.get(1) {
                    let path_str = path_match.as_str().replace("\\\\", "\\");
                    let path = PathBuf::from(path_str);
                    if path.exists() && !folders.contains(&path) {
                        folders.push(path);
                    }
                }
            }
        }
    }

    Ok(folders)
}

fn parse_appmanifest_basic(manifest_path: &PathBuf) -> Option<(u32, String)> {
    if let Ok(contents) = std::fs::read_to_string(manifest_path) {
        let app_id_re = regex::Regex::new(r#""appid"\s+"(\d+)""#).ok()?;
        let name_re = regex::Regex::new(r#""name"\s+"([^"]+)""#).ok()?;

        let app_id = app_id_re.captures(&contents)
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse::<u32>().ok())?;

        let name = name_re.captures(&contents)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())?;

        Some((app_id, name))
    } else {
        None
    }
}

async fn handle_game_backup(
    game_name: String,
    state: &AppState,
    app_handle: tauri::AppHandle,
) {
    println!("Backing up: {}", game_name);
    
    let (ludusavi_path, backup_path, notifications_enabled) = {
        let config = state.config.lock().unwrap();
        let cfg = config.get_all();
        (cfg.ludusavi_path, cfg.backup_path, cfg.notifications_enabled)
    };
    
    let manager = LudusaviManager::new(ludusavi_path, backup_path);
    
    match manager.backup(&game_name).await {
        Ok(result) => {
            if result.success {
                if notifications_enabled {
                    state.notification_manager.lock().unwrap().show_backup_success(
                        &game_name,
                        result.files_backed_up.unwrap_or(0),
                        &result.total_size.unwrap_or_default(),
                    );
                }
            } else if result.not_found.unwrap_or(false) {
                if notifications_enabled {
                    state.notification_manager.lock().unwrap().show_game_not_found(&game_name);
                }

                // Send to frontend
                let _ = app_handle.emit_all("game-not-found", serde_json::json!({ "name": game_name }));
            } else {
                if notifications_enabled {
                    state.notification_manager.lock().unwrap().show_backup_failed(
                        &game_name,
                        &result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Backup error: {}", e);
            if notifications_enabled {
                state.notification_manager.lock().unwrap().show_error("Backup Error", &format!("Error backing up {}", game_name));
            }
        }
    }
}

async fn start_monitors(state: &AppState, window: Window) {
    println!("Starting monitors...");

    // Check if monitors are already running
    {
        let steam_handle = state.steam_handle.lock().unwrap();
        if steam_handle.is_some() {
            println!("WARNING: Steam monitor already running! Skipping start to prevent duplicates.");
            return;
        }
    }

    let config = {
        let cfg = state.config.lock().unwrap();
        cfg.get_all()
    };

    if config.ludusavi_path.is_empty() || config.backup_path.is_empty() {
        println!("Configuration incomplete, skipping monitor initialization");
        return;
    }

    let app_handle = window.app_handle();
    
    // Start Steam monitor (monitors localconfig.vdf file)
    // No API keys or Steamworks required!
    match SteamMonitor::new() {
        Ok(mut monitor) => {
            // Set database path for exclusions checking
            if let Some(ref db_path) = *state.achievement_db_path.lock().unwrap() {
                monitor.set_db_path(db_path.clone());
            }

            let (tx, mut rx) = mpsc::channel(10);
            let state_clone = state.clone();
            let app_clone = app_handle.clone();

            tokio::spawn(async move {
                let mut monitor = monitor;
                let mut paused = false;

                loop {
                    tokio::select! {
                        // Check for commands
                        Some(cmd) = rx.recv() => {
                            match cmd {
                                MonitorCommand::Stop => {
                                    println!("Steamworks monitor stopped");
                                    break;
                                }
                                MonitorCommand::Pause => {
                                    println!("Steamworks monitor paused");
                                    paused = true;
                                }
                                MonitorCommand::Resume => {
                                    println!("Steamworks monitor resumed");
                                    paused = false;
                                }
                            }
                        }
                        // Check Steam if not paused
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                            if !paused {
                                if let Some(event) = monitor.check_steam() {
                                    match event {
                                        steam_monitor::GameEvent::Ended(game) => {
                                            println!("Steam game ended: {}", game.name);

                                            // Stop watching achievements for this game
                                            if let Some(ref watcher) = *state_clone.achievement_watcher.lock().unwrap() {
                                                watcher.stop_watching_game(game.app_id);
                                            }

                                            handle_game_backup(game.name, &state_clone, app_clone.clone()).await;
                                        }
                                        steam_monitor::GameEvent::Started(game) => {
                                            println!("Steam game started: {}", game.name);

                                            // Start watching achievements for this game
                                            if let Some(ref watcher) = *state_clone.achievement_watcher.lock().unwrap() {
                                                let watcher = Arc::clone(watcher);
                                                let app_id = game.app_id;
                                                let game_name = game.name.clone();
                                                tokio::spawn(async move {
                                                    watcher.start_watching_game(app_id, game_name).await;
                                                });
                                            }

                                            // Get notification settings
                                            let notifications_enabled = {
                                                let config = state_clone.config.lock().unwrap();
                                                config.get_all().notifications_enabled
                                            };

                                            if notifications_enabled {
                                                state_clone.notification_manager.lock().unwrap().show_game_detected(&game.name);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });

            *state.steam_handle.lock().unwrap() = Some(tx);
            println!("✓ Steam monitoring started (no API key needed!)");
        }
        Err(e) => {
            println!("⚠ Steam not available: {}. Steam monitoring disabled.", e);
            println!("   Make sure Steam is installed to enable automatic game detection.");
        }
    }
    
    // Start process monitor
    if !config.game_executables.is_empty() {
        let (tx, mut rx) = mpsc::channel(1);
        let game_exes = config.game_executables.clone();
        let state_clone = state.clone();
        let app_clone = app_handle.clone();
        let notifications = config.notifications_enabled;
        
        tokio::spawn(async move {
            let mut monitor = ProcessMonitor::new(game_exes);
            
            tokio::select! {
                _ = async {
                    loop {
                        if let Some(event) = monitor.check_processes().await {
                            match event {
                                process_monitor::GameEvent::Started(game) => {
                                    println!("Process-monitored game detected: {}", game.name);
                                    
                                    // Pause Steam monitoring
                                    let steam_tx_opt = {
                                        let guard = state_clone.steam_handle.lock().unwrap();
                                        guard.clone()
                                    };
                                    
                                    if let Some(steam_tx) = steam_tx_opt {
                                        let _ = steam_tx.send(MonitorCommand::Pause).await;
                                        println!("Paused Steam monitoring while {} is running", game.name);
                                    }
                                    
                                    if notifications {
                                        state_clone.notification_manager.lock().unwrap().show_game_detected(&game.name);
                                    }
                                    
                                    let _ = app_clone.emit_all("game-detected", &game.name);
                                }
                                process_monitor::GameEvent::Ended(game) => {
                                    println!("Process-monitored game ended: {}", game.name);
                                    
                                    // Resume Steam monitoring
                                    let steam_tx_opt = {
                                        let guard = state_clone.steam_handle.lock().unwrap();
                                        guard.clone()
                                    };
                                    
                                    if let Some(steam_tx) = steam_tx_opt {
                                        let _ = steam_tx.send(MonitorCommand::Resume).await;
                                        println!("Resumed Steam monitoring");
                                    }
                                    
                                    if notifications {
                                        state_clone.notification_manager.lock().unwrap().show_game_ended(&game.name);
                                    }
                                    
                                    handle_game_backup(game.name, &state_clone, app_clone.clone()).await;
                                }
                            }
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    }
                } => {}
                _ = rx.recv() => {
                    println!("Process monitor stopped");
                }
            }
        });

        *state.process_handle.lock().unwrap() = Some(tx);
        println!("✓ Process monitor started for {} games", config.game_executables.len());
    }

    println!("All monitors started successfully");
}

async fn stop_monitors(state: &AppState) {
    println!("Stopping monitors...");

    // Stop all achievement watchers first to prevent duplicate notifications
    if let Some(ref watcher) = *state.achievement_watcher.lock().unwrap() {
        watcher.stop_all_watchers();
    }

    // Stop Steam monitor
    let steam_tx = state.steam_handle.lock().unwrap().take();
    if let Some(tx) = steam_tx {
        println!("Sending stop command to Steam monitor");
        let _ = tx.send(MonitorCommand::Stop).await;
    }

    // Stop process monitor
    let process_tx = state.process_handle.lock().unwrap().take();
    if let Some(tx) = process_tx {
        println!("Sending stop command to process monitor");
        let _ = tx.send(true).await;
    }

    // Give monitors more time to shut down gracefully and complete any in-progress operations
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    println!("Monitors stopped");
}

fn create_tray() -> SystemTray {
    let open = CustomMenuItem::new("open".to_string(), "Open Settings");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let tray_menu = SystemTrayMenu::new()
        .add_item(open)
        .add_native_item(tauri::SystemTrayMenuItem::Separator)
        .add_item(quit);
    
    SystemTray::new().with_menu(tray_menu)
}

fn main() {
    // Set up panic hook to write to file and show message box
    std::panic::set_hook(Box::new(|panic_info| {
        let panic_msg = format!("PANIC: {:?}", panic_info);
        eprintln!("{}", panic_msg);

        // Write to log file in Documents folder
        if let Some(docs) = dirs::document_dir() {
            let log_path = docs.join("Steam Backup Manager Crash.log");
            let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
            let log_msg = format!("[{}] {}\n", timestamp, panic_msg);
            let _ = std::fs::write(&log_path, log_msg);

            // Show message box
            #[cfg(windows)]
            {
                use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK, MB_ICONERROR};
                use windows::core::PCWSTR;
                unsafe {
                    let title: Vec<u16> = "Steam Backup Manager Crash"
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    let msg: Vec<u16> = format!("App crashed! Error log saved to:\n{}\n\nError: {}",
                        log_path.display(), panic_msg)
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    MessageBoxW(None, PCWSTR(msg.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
                }
            }
        }
    }));

    // Also set up file logging for regular messages
    if let Some(docs) = dirs::document_dir() {
        let log_path = docs.join("Steam Backup Manager Debug.log");
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
        let _ = std::fs::write(&log_path, format!("[{}] App starting...\n", timestamp));
        println!("Logging to: {}", log_path.display());
    }

    tauri::Builder::default()
        .setup(|app| {
            // CRITICAL: Register state IMMEDIATELY with minimal setup
            // This prevents race conditions where frontend tries to access state before it's ready
            let config = Arc::new(Mutex::new(ConfigManager::new()));

            // Create state with MINIMAL initialization - don't initialize anything yet!
            let achievement_duration = Arc::new(Mutex::new(6)); // Default 6 seconds

            let state = AppState {
                config: config.clone(),
                steam_handle: Arc::new(Mutex::new(None)),
                process_handle: Arc::new(Mutex::new(None)),
                notification_manager: Arc::new(Mutex::new(NotificationManager::new(achievement_duration.clone()))),
                achievement_db_path: Arc::new(Mutex::new(None)),
                achievement_watcher: Arc::new(Mutex::new(None)),
                overlay_manager: Arc::new(Mutex::new(OverlayManager::new())),
                achievement_duration,
            };

            // Register state FIRST - before doing ANYTHING else
            app.manage(state.clone());
            println!("✓ State registered with Tauri (frontend can now access it safely)");

            // NOW create and show the main window - state is registered so frontend can safely call commands
            let main_window = tauri::WindowBuilder::new(
                app,
                "main",
                tauri::WindowUrl::App("index.html".into())
            )
            .title("Steam Backup Manager")
            .inner_size(1100.0, 800.0)
            .resizable(true)
            .center()
            .build()
            .map_err(|e| format!("Failed to create main window: {}", e))?;
            println!("✓ Main window created and shown");

            // Now it's safe to initialize components
            // Initialize overlay manager
            {
                let mut overlay = state.overlay_manager.lock().unwrap();
                if let Err(e) = overlay.init(&app.app_handle()) {
                    eprintln!("Failed to initialize overlay: {}", e);
                } else {
                    println!("✓ Overlay initialized");
                }
            }

            // Set overlay in notification manager
            {
                let mut notif = state.notification_manager.lock().unwrap();
                notif.set_overlay_manager(state.overlay_manager.clone());
                println!("✓ Notification manager configured");
            }

            // Listen for overlay-notifications-done event to auto-hide overlay
            let overlay_manager_for_listener = state.overlay_manager.clone();
            if let Some(overlay_window) = app.get_window("overlay") {
                overlay_window.listen("overlay-notifications-done", move |_event| {
                    println!("[Overlay] Received notifications-done event, hiding overlay");
                    if let Ok(overlay) = overlay_manager_for_listener.lock() {
                        let _ = overlay.hide_overlay();
                    }
                });

                // IMPORTANT: Send initial settings to overlay window
                // This ensures the overlay has the correct settings even in production builds
                // where localStorage is NOT shared between windows
                println!("[Overlay] Sending initial settings to overlay window");

                // Send achievement settings (duration)
                let achievement_settings = serde_json::json!({ "duration": 6 }); // Default value
                if let Err(e) = overlay_window.emit("achievement-settings-sync", &achievement_settings) {
                    eprintln!("Failed to emit initial achievement settings: {}", e);
                }

                // Send rarity settings
                let rarity_settings = serde_json::json!({
                    "enabled": false,
                    "Common": {
                        "backgroundColor": "#1f2937",
                        "borderColor": "#6b7280",
                        "textColor": "#ffffff",
                        "soundPath": null,
                        "customFont": null
                    },
                    "Uncommon": {
                        "backgroundColor": "#14532d",
                        "borderColor": "#16a34a",
                        "textColor": "#ffffff",
                        "soundPath": null,
                        "customFont": null
                    },
                    "Rare": {
                        "backgroundColor": "#1e3a8a",
                        "borderColor": "#3b82f6",
                        "textColor": "#ffffff",
                        "soundPath": null,
                        "customFont": null
                    },
                    "Ultra Rare": {
                        "backgroundColor": "#581c87",
                        "borderColor": "#a855f7",
                        "textColor": "#ffffff",
                        "soundPath": null,
                        "customFont": null
                    },
                    "Legendary": {
                        "backgroundColor": "#78350f",
                        "borderColor": "#f59e0b",
                        "textColor": "#ffffff",
                        "soundPath": null,
                        "customFont": null
                    }
                });
                if let Err(e) = overlay_window.emit("rarity-settings-sync", &rarity_settings) {
                    eprintln!("Failed to emit initial rarity settings: {}", e);
                }
            }

            // Initialize achievement database
            let db_path = app.path_resolver()
                .app_data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("achievements.db");

            // Create parent directory if it doesn't exist
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            // Verify database can be created, then close it
            let achievement_db_path_option = match AchievementDatabase::new(db_path.clone()) {
                Ok(_db) => {
                    println!("✓ Achievement database initialized at: {}", db_path.display());
                    Some(db_path.clone())
                }
                Err(e) => {
                    eprintln!("⚠ Failed to initialize achievement database: {}", e);
                    None
                }
            };

            // Update state with database path
            *state.achievement_db_path.lock().unwrap() = achievement_db_path_option.clone();

            // Initialize achievement watcher
            let steam_path = PathBuf::from(r"C:\Program Files (x86)\Steam");
            let steam_user_id_for_watcher = {
                let config_guard = config.lock().unwrap();
                let cfg = config_guard.get_all();
                cfg.steam_user_id
            };
            let achievement_watcher_option = achievement_db_path_option.as_ref().map(|_| {
                // Create steam client for the watcher
                let (api_key, steam_id_64) = {
                    let config_guard = config.lock().unwrap();
                    let cfg = config_guard.get_all();
                    (cfg.steam_api_key, cfg.steam_id_64)
                };
                let steam_client = Arc::new(
                    SteamAchievementClient::new(api_key, steam_id_64)
                        .expect("Failed to create steam client for achievement watcher")
                );

                let mut watcher = AchievementWatcher::new(db_path.clone(), steam_path.clone(), steam_user_id_for_watcher, state.notification_manager.clone(), steam_client);

                // Create channel for achievement unlock events
                let (unlock_tx, unlock_rx) = channel::<AchievementUnlockEvent>();
                watcher.set_event_sender(unlock_tx);

                // Spawn task to listen for achievement unlock events and emit them to frontend
                let app_handle = app.app_handle();
                std::thread::spawn(move || {
                    while let Ok(event) = unlock_rx.recv() {
                        println!("🏆 Achievement unlocked: {} - {}", event.game_name, event.display_name);
                        let _ = app_handle.emit_all("achievement-unlocked", &event);
                    }
                });

                Arc::new(watcher)
            });

            // Update state with achievement watcher
            *state.achievement_watcher.lock().unwrap() = achievement_watcher_option;

            // Initialize monitors
            let state_clone = state.clone();
            let window_clone = main_window.clone();
            tauri::async_runtime::spawn(async move {
                start_monitors(&state_clone, window_clone).await;
            });

            // Start periodic checking for pending games (every 10 minutes)
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(600)); // 10 minutes
                loop {
                    interval.tick().await;

                    // Clone watcher Arc in a separate block to drop the mutex guard
                    let watcher_opt = {
                        let guard = state_clone.achievement_watcher.lock().unwrap();
                        guard.as_ref().map(|w| Arc::clone(w))
                    };

                    if let Some(watcher) = watcher_opt {
                        watcher.check_pending_games().await;
                    }
                }
            });

            Ok(())
        })
        .system_tray(create_tray())
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                let window = app.get_window("main").unwrap();
                window.show().unwrap();
                window.set_focus().unwrap();
            }
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "open" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            browse_file,
            browse_folder,
            test_ludusavi,
            get_ludusavi_manifest,
            get_all_achievements,
            get_game_achievements,
            update_achievement_status,
            sync_achievements,
            add_manual_achievement,
            export_achievements,
            export_game_achievements,
            search_steam_games,
            check_game_sources,
            add_game_from_source,
            remove_game_from_tracking,
            get_all_exclusions,
            add_exclusion,
            remove_exclusion,
            fetch_achievement_icon,
            test_overlay,
            test_rarity_notification,
            sync_settings_to_overlay,
            get_achievement_duration,
            set_achievement_duration,
            play_windows_notification_sound,
            debug_log,
            read_audio_file,
            check_backup_exists,
            restore_from_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}