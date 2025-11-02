use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use crate::achievements::{Achievement, AchievementDatabase};
use crate::achievement_scanner::AchievementScanner;
use crate::steam_achievements::SteamAchievementClient;
use crate::notifications::NotificationManager;
use std::collections::HashMap as StdHashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementUnlockEvent {
    pub app_id: u32,
    pub game_name: String,
    pub achievement_id: String,
    pub display_name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub unlock_time: i64,
    pub source: String,
    pub global_unlock_percentage: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct GameAchievementSource {
    pub app_id: u32,
    pub game_name: String,
    pub file_path: PathBuf,
    pub source_type: AchievementSourceType,
}

#[derive(Debug, Clone)]
pub enum AchievementSourceType {
    OnlineFix,
    LibraryCache,
    Goldberg,
    SteamWebApi,
}

impl std::fmt::Display for AchievementSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AchievementSourceType::OnlineFix => write!(f, "Online-fix"),
            AchievementSourceType::LibraryCache => write!(f, "Steamtools"),
            AchievementSourceType::Goldberg => write!(f, "Goldberg"),
            AchievementSourceType::SteamWebApi => write!(f, "Steam Web API"),
        }
    }
}

pub struct AchievementWatcher {
    watchers: Arc<Mutex<HashMap<u32, RecommendedWatcher>>>,
    watched_games: Arc<Mutex<HashMap<u32, GameAchievementSource>>>,
    pending_games: Arc<Mutex<HashMap<u32, (String, SystemTime)>>>, // app_id -> (game_name, last_check_time)
    db_path: PathBuf,
    steam_path: PathBuf,
    steam_user_id: Option<String>,
    event_sender: Option<Sender<AchievementUnlockEvent>>,
    notification_manager: Arc<Mutex<NotificationManager>>,
    steam_client: Arc<SteamAchievementClient>,
}

impl AchievementWatcher {
    pub fn new(db_path: PathBuf, steam_path: PathBuf, steam_user_id: Option<String>, notification_manager: Arc<Mutex<NotificationManager>>, steam_client: Arc<SteamAchievementClient>) -> Self {
        Self {
            watchers: Arc::new(Mutex::new(HashMap::new())),
            watched_games: Arc::new(Mutex::new(HashMap::new())),
            pending_games: Arc::new(Mutex::new(HashMap::new())),
            db_path,
            steam_path,
            steam_user_id,
            event_sender: None,
            notification_manager,
            steam_client,
        }
    }

    pub fn set_event_sender(&mut self, sender: Sender<AchievementUnlockEvent>) {
        self.event_sender = Some(sender);
    }

    /// Find achievement source for a game using the priority: OnlineFix → librarycache → goldberg → steam web api
    pub fn find_achievement_source(&self, app_id: u32, game_name: &str) -> Option<GameAchievementSource> {
        // Exclude Borderless Gaming (AppID 388080) from achievement monitoring
        if app_id == 388080 {
            println!("  ⊘ Skipping Borderless Gaming (AppID 388080) - excluded from monitoring");
            return None;
        }

        // Priority 1: OnlineFix
        let onlinefix_base = PathBuf::from(r"C:\Users\Public\Documents\OnlineFix")
            .join(format!("{}", app_id));

        let onlinefix_path = if onlinefix_base.join("Stats").join("Achievements.ini").exists() {
            Some(onlinefix_base.join("Stats").join("Achievements.ini"))
        } else if onlinefix_base.join("stats").join("Achievements.ini").exists() {
            Some(onlinefix_base.join("stats").join("Achievements.ini"))
        } else if onlinefix_base.join("Stats").join("achievements.ini").exists() {
            Some(onlinefix_base.join("Stats").join("achievements.ini"))
        } else if onlinefix_base.join("stats").join("achievements.ini").exists() {
            Some(onlinefix_base.join("stats").join("achievements.ini"))
        } else {
            None
        };

        if let Some(path) = onlinefix_path {
            println!("  ✓ Found OnlineFix achievements for {} at: {:?}", game_name, path);
            return Some(GameAchievementSource {
                app_id,
                game_name: game_name.to_string(),
                file_path: path,
                source_type: AchievementSourceType::OnlineFix,
            });
        }

        // Priority 2: LibraryCache - use configured Steam user ID
        if let Some(ref user_id) = self.steam_user_id {
            let userdata_path = self.steam_path.join("userdata").join(user_id);
            let librarycache_path = userdata_path
                .join("config")
                .join("librarycache")
                .join(format!("{}.json", app_id));

            if librarycache_path.exists() {
                println!("  ✓ Found LibraryCache achievements for {} at: {:?}", game_name, librarycache_path);
                return Some(GameAchievementSource {
                    app_id,
                    game_name: game_name.to_string(),
                    file_path: librarycache_path,
                    source_type: AchievementSourceType::LibraryCache,
                });
            }
        }

        // Priority 3: Goldberg (GSE Saves)
        let appdata = std::env::var("APPDATA").ok()?;
        let goldberg_paths = vec![
            PathBuf::from(&appdata).join("GSE Saves").join(format!("{}", app_id)).join("achievements.json"),
            PathBuf::from(&appdata).join("Goldberg SteamEmu Saves").join(format!("{}", app_id)).join("achievements.json"),
        ];

        for path in goldberg_paths {
            if path.exists() {
                println!("  ✓ Found Goldberg achievements for {} at: {:?}", game_name, path);
                return Some(GameAchievementSource {
                    app_id,
                    game_name: game_name.to_string(),
                    file_path: path,
                    source_type: AchievementSourceType::Goldberg,
                });
            }
        }

        // Priority 4: Steam Web API (no file to watch, will be handled differently)
        println!("  ℹ No local achievement files found for {}. Will use Steam Web API polling.", game_name);
        None
    }

    fn find_steam_userdata(&self) -> Result<PathBuf, String> {
        let userdata_path = self.steam_path.join("userdata");

        if !userdata_path.exists() {
            return Err("Steam userdata folder not found".to_string());
        }

        let user_dirs: Vec<_> = std::fs::read_dir(&userdata_path)
            .map_err(|e| format!("Failed to read userdata: {}", e))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().is_dir()
                    && entry.file_name() != "0"
                    && entry.file_name() != "ac"
            })
            .collect();

        if user_dirs.is_empty() {
            return Err("No Steam user found".to_string());
        }

        Ok(user_dirs[0].path())
    }

    /// Find the file for a specific source by name
    fn find_specific_source(&self, app_id: u32, game_name: &str, source_name: &str) -> Option<GameAchievementSource> {
        println!("  🔍 Looking for {} file...", source_name);

        match source_name {
            "Online-fix" => {
                let onlinefix_base = PathBuf::from(r"C:\Users\Public\Documents\OnlineFix")
                    .join(format!("{}", app_id));

                let paths = vec![
                    onlinefix_base.join("Stats").join("Achievements.ini"),
                    onlinefix_base.join("stats").join("Achievements.ini"),
                    onlinefix_base.join("Stats").join("achievements.ini"),
                    onlinefix_base.join("stats").join("achievements.ini"),
                ];

                for path in paths {
                    println!("    Checking: {:?}", path);
                    if path.exists() {
                        return Some(GameAchievementSource {
                            app_id,
                            game_name: game_name.to_string(),
                            file_path: path,
                            source_type: AchievementSourceType::OnlineFix,
                        });
                    }
                }
            }
            "Steamtools" => {
                if let Some(ref user_id) = self.steam_user_id {
                    println!("    Using configured Steam user ID: {}", user_id);
                    let userdata_path = self.steam_path.join("userdata").join(user_id);
                    println!("    Userdata path: {:?}", userdata_path);

                    let librarycache_path = userdata_path
                        .join("config")
                        .join("librarycache")
                        .join(format!("{}.json", app_id));

                    println!("    Checking: {:?}", librarycache_path);
                    if librarycache_path.exists() {
                        println!("    ✓ File exists!");
                        return Some(GameAchievementSource {
                            app_id,
                            game_name: game_name.to_string(),
                            file_path: librarycache_path,
                            source_type: AchievementSourceType::LibraryCache,
                        });
                    } else {
                        println!("    ✗ File does not exist at this path");
                    }
                } else {
                    println!("    ✗ No Steam user ID configured in settings!");
                }
            }
            "Goldberg" => {
                if let Ok(appdata) = std::env::var("APPDATA") {
                    let goldberg_paths = vec![
                        PathBuf::from(&appdata).join("GSE Saves").join(format!("{}", app_id)).join("achievements.json"),
                        PathBuf::from(&appdata).join("Goldberg SteamEmu Saves").join(format!("{}", app_id)).join("achievements.json"),
                    ];

                    for path in goldberg_paths {
                        if path.exists() {
                            return Some(GameAchievementSource {
                                app_id,
                                game_name: game_name.to_string(),
                                file_path: path,
                                source_type: AchievementSourceType::Goldberg,
                            });
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }

    /// Start watching achievement file for a game
    pub async fn start_watching_game(&self, app_id: u32, game_name: String) {
        println!("🔍 Looking for achievement source for {} (AppID: {})...", game_name, app_id);

        // FIRST: Check database to see what source this game was added with
        if let Ok(db) = AchievementDatabase::new(self.db_path.clone()) {
            if let Ok(achievements) = db.get_game_achievements(app_id) {
                if let Some(first_ach) = achievements.first() {
                    let db_source = &first_ach.source;
                    println!("  📋 Game was added with source: {}", db_source);

                    // Find the file for this specific source
                    if let Some(source) = self.find_specific_source(app_id, &game_name, db_source) {
                        println!("  ✓ Will monitor {} for achievements", db_source);
                        self.setup_file_watcher(source.clone(), self.steam_client.clone()).await;

                        // Store in watched games
                        {
                            let mut watched = self.watched_games.lock().unwrap();
                            watched.insert(app_id, source);
                        }
                        return;
                    } else {
                        println!("  ⚠ Cannot find {} file for monitoring", db_source);
                    }
                }
            }
        }

        // FALLBACK: If not in database, use priority search
        if let Some(source) = self.find_achievement_source(app_id, &game_name) {
            // Found a source, set up file watcher
            self.setup_file_watcher(source.clone(), self.steam_client.clone()).await;

            // Store in watched games
            {
                let mut watched = self.watched_games.lock().unwrap();
                watched.insert(app_id, source);
            }
        } else {
            // No source found, add to pending list for periodic checking
            {
                let mut pending = self.pending_games.lock().unwrap();
                pending.insert(app_id, (game_name.clone(), SystemTime::now()));
            }
            println!("  ⏱ Will check periodically every 10 minutes for {} until a source is found.", game_name);
        }
    }

    /// Stop watching achievement file for a game
    pub fn stop_watching_game(&self, app_id: u32) {
        // Remove from watchers
        let mut watchers = self.watchers.lock().unwrap();
        if let Some(_watcher) = watchers.remove(&app_id) {
            println!("  ✓ Stopped watching achievements for AppID: {}", app_id);
        }

        // Remove from watched games
        let mut watched = self.watched_games.lock().unwrap();
        watched.remove(&app_id);

        // Remove from pending games
        let mut pending = self.pending_games.lock().unwrap();
        pending.remove(&app_id);
    }

    /// Set up file watcher for an achievement source
    async fn setup_file_watcher(&self, source: GameAchievementSource, steam_client: Arc<SteamAchievementClient>) {
        let app_id = source.app_id;
        let file_path = source.file_path.clone();
        let db_path = self.db_path.clone();
        let event_sender = self.event_sender.clone();
        let source_type = source.source_type.clone();
        let game_name = source.game_name.clone();
        let notification_manager = self.notification_manager.clone();

        // Create a channel to receive file system events
        let (tx, rx): (Sender<Result<Event, notify::Error>>, Receiver<Result<Event, notify::Error>>) = channel();

        // Create file watcher
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                println!("  ✗ Failed to create watcher for {}: {}", game_name, e);
                return;
            }
        };

        // Watch the file
        if let Err(e) = watcher.watch(&file_path, RecursiveMode::NonRecursive) {
            println!("  ✗ Failed to watch file {:?}: {}", file_path, e);
            return;
        }

        println!("  ✓ Watching {} achievements at: {:?}", source_type, file_path);

        // Store watcher
        {
            let mut watchers = self.watchers.lock().unwrap();
            watchers.insert(app_id, watcher);
        }

        // Spawn task to handle file change events
        let steam_path = self.steam_path.clone();
        tokio::spawn(async move {
            while let Ok(res) = rx.recv() {
                match res {
                    Ok(event) => {
                        // Process modify, create, and write events (Windows sends different events)
                        if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Access(_)) {
                            println!("  📝 Achievement file change detected for AppID: {} ({:?})", app_id, event.kind);

                            // Give the file a moment to finish writing (longer for JSON files)
                            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                            // Check for unlocks
                            if let Err(e) = Self::check_for_unlocks(
                                app_id,
                                &game_name,
                                &file_path,
                                &source_type,
                                &db_path,
                                &steam_path,
                                event_sender.clone(),
                                notification_manager.clone(),
                                steam_client.clone(),
                            ).await {
                                println!("  ✗ Error checking for unlocks: {}", e);
                            }
                        }
                    }
                    Err(e) => println!("  ✗ Watch error: {}", e),
                }
            }
        });
    }

    /// Check for achievement unlocks by comparing file state vs database
    async fn check_for_unlocks(
        app_id: u32,
        game_name: &str,
        file_path: &PathBuf,
        source_type: &AchievementSourceType,
        db_path: &PathBuf,
        steam_path: &PathBuf,
        event_sender: Option<Sender<AchievementUnlockEvent>>,
        notification_manager: Arc<Mutex<NotificationManager>>,
        steam_client: Arc<SteamAchievementClient>,
    ) -> Result<(), String> {
        // Get current achievements from database
        let db = AchievementDatabase::new(db_path.clone())?;
        let db_achievements = db.get_game_achievements(app_id)?;

        // Create a lookup map for quick access
        let mut db_map: HashMap<String, Achievement> = HashMap::new();
        for ach in &db_achievements {
            db_map.insert(ach.achievement_id.clone(), ach.clone());
        }

        // Parse current file state and detect unlocks
        let unlocked_achievements = match source_type {
            AchievementSourceType::OnlineFix => {
                Self::parse_onlinefix_unlocks(file_path, &db_map)?
            }
            AchievementSourceType::LibraryCache => {
                Self::parse_librarycache_unlocks(file_path, &db_map)?
            }
            AchievementSourceType::Goldberg => {
                Self::parse_goldberg_unlocks(file_path, &db_map)?
            }
            AchievementSourceType::SteamWebApi => {
                // This shouldn't happen as Steam Web API doesn't have a file to watch
                return Ok(());
            }
        };

        // Fetch global percentages for all achievements in this game (once per unlock detection)
        println!("  📊 Fetching global achievement percentages from Steam API for app_id {}...", app_id);
        let global_percentages = match steam_client.get_global_achievement_percentages(app_id).await {
            Ok(percentages) => {
                println!("  ✓ Retrieved global achievement percentages for {} achievements", percentages.len());
                println!("  DEBUG: Available achievement IDs: {:?}", percentages.keys().take(10).collect::<Vec<_>>());
                Some(percentages)
            }
            Err(e) => {
                println!("  ❌ ERROR fetching global percentages: {}", e);
                None
            }
        };

        // Update database and emit events for newly unlocked achievements
        for (achievement_id, unlock_time) in unlocked_achievements {
            if let Some(db_ach) = db_map.get(&achievement_id) {
                if !db_ach.achieved {
                    // Achievement was just unlocked!
                    println!("  🏆 Achievement unlocked: {} - {}", game_name, db_ach.display_name);
                    println!("  DEBUG: Looking up percentage for achievement_id: '{}'", achievement_id);

                    // Get global unlock percentage for this specific achievement
                    let global_percentage = global_percentages.as_ref()
                        .and_then(|percentages| percentages.get(&achievement_id))
                        .copied();

                    if let Some(pct) = global_percentage {
                        println!("  ✅ Global unlock rate: {:.1}%", pct);
                    } else {
                        println!("  ❌ No percentage found for achievement_id: '{}'", achievement_id);
                    }

                    // Update database with achieved status AND global percentage
                    if let Some(id) = db_ach.id {
                        db.update_achievement_status(id, true, Some(unlock_time))?;

                        // Also update the global percentage if we fetched it
                        if global_percentage.is_some() && db_ach.global_unlock_percentage.is_none() {
                            // Re-fetch the achievement to update its global percentage
                            let mut updated_ach = db_ach.clone();
                            updated_ach.global_unlock_percentage = global_percentage;
                            db.insert_or_update_achievement(&updated_ach)?;
                        }
                    }

                    // Show overlay notification (or Windows native as fallback) with the fetched percentage
                    notification_manager.lock().unwrap().show_achievement_unlock(
                        game_name,
                        &db_ach.display_name,
                        &db_ach.description,
                        db_ach.icon_url.as_deref(),
                        global_percentage.or(db_ach.global_unlock_percentage)
                    );

                    // Emit event for in-app toast notification
                    if let Some(ref sender) = event_sender {
                        let event = AchievementUnlockEvent {
                            app_id,
                            game_name: game_name.to_string(),
                            achievement_id: achievement_id.clone(),
                            display_name: db_ach.display_name.clone(),
                            description: db_ach.description.clone(),
                            icon_url: db_ach.icon_url.clone(),
                            unlock_time,
                            source: source_type.to_string(),
                            global_unlock_percentage: global_percentage.or(db_ach.global_unlock_percentage),
                        };
                        let _ = sender.send(event);
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse OnlineFix achievements file for unlocks
    fn parse_onlinefix_unlocks(
        file_path: &PathBuf,
        _db_map: &HashMap<String, Achievement>,
    ) -> Result<Vec<(String, i64)>, String> {
        let contents = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read OnlineFix file: {}", e))?;

        let section_regex = regex::Regex::new(r"(?m)^\[([^\]]+)\]")
            .map_err(|e| format!("Failed to create regex: {}", e))?;
        let achieved_regex = regex::Regex::new(r"(?m)^achieved\s*=\s*(\w+)")
            .map_err(|e| format!("Failed to create regex: {}", e))?;
        let timestamp_regex = regex::Regex::new(r"(?m)^timestamp\s*=\s*(\d+)")
            .map_err(|e| format!("Failed to create regex: {}", e))?;

        let mut unlocked = Vec::new();

        for section_cap in section_regex.captures_iter(&contents) {
            let section_match = section_cap.get(0).unwrap();
            let section_name = section_cap.get(1).unwrap().as_str();

            let section_start = section_match.end();
            let next_section_pos = contents[section_start..]
                .find("\n[")
                .map(|pos| section_start + pos)
                .unwrap_or(contents.len());

            let section_content = &contents[section_start..next_section_pos];

            let achieved = if let Some(ach_cap) = achieved_regex.captures(section_content) {
                ach_cap.get(1).map(|m| m.as_str().to_lowercase() == "true").unwrap_or(false)
            } else {
                false
            };

            if achieved {
                let unlock_time = if let Some(ts_cap) = timestamp_regex.captures(section_content) {
                    ts_cap.get(1)
                        .and_then(|m| m.as_str().parse::<i64>().ok())
                        .filter(|&t| t > 0)
                        .unwrap_or_else(|| chrono::Utc::now().timestamp())
                } else {
                    chrono::Utc::now().timestamp()
                };

                unlocked.push((section_name.to_string(), unlock_time));
            }
        }

        Ok(unlocked)
    }

    /// Parse LibraryCache achievements file for unlocks
    fn parse_librarycache_unlocks(
        file_path: &PathBuf,
        _db_map: &HashMap<String, Achievement>,
    ) -> Result<Vec<(String, i64)>, String> {
        println!("  🔍 Parsing library cache file: {:?}", file_path);

        let contents = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read LibraryCache file: {}", e))?;

        let json: serde_json::Value = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let achievements_entry = json.as_array()
            .and_then(|arr| {
                arr.iter().find(|item| {
                    item.as_array()
                        .and_then(|inner| inner.get(0))
                        .and_then(|v| v.as_str())
                        .map(|s| s == "achievements")
                        .unwrap_or(false)
                })
            })
            .ok_or_else(|| "No achievements entry found".to_string())?;

        let achievement_data = achievements_entry.as_array()
            .and_then(|arr| arr.get(1))
            .and_then(|v| v.get("data"))
            .ok_or_else(|| "Invalid achievement data structure".to_string())?;

        let mut unlocked = Vec::new();

        // Process vecHighlight
        if let Some(vec_highlight) = achievement_data.get("vecHighlight").and_then(|v| v.as_array()) {
            println!("  📋 Found {} achievements in vecHighlight", vec_highlight.len());
            for ach in vec_highlight {
                let achievement_id = ach.get("strID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let achieved = ach.get("bAchieved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let unlock_time = ach.get("rtUnlocked")
                    .and_then(|v| v.as_i64())
                    .filter(|&t| t > 0)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp());

                if achieved {
                    if let Some(id) = achievement_id {
                        println!("  ✓ Found unlocked: {} at {}", id, unlock_time);
                        unlocked.push((id, unlock_time));
                    }
                }
            }
        }

        // Process vecAchievedHidden
        if let Some(vec_achieved_hidden) = achievement_data.get("vecAchievedHidden").and_then(|v| v.as_array()) {
            println!("  📋 Found {} achievements in vecAchievedHidden", vec_achieved_hidden.len());
            for ach in vec_achieved_hidden {
                let achievement_id = ach.get("strID")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let achieved = ach.get("bAchieved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true); // Default to true for vecAchievedHidden

                let unlock_time = ach.get("rtUnlocked")
                    .and_then(|v| v.as_i64())
                    .filter(|&t| t > 0)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp());

                if achieved {
                    if let Some(id) = achievement_id {
                        println!("  ✓ Found unlocked (hidden): {} at {}", id, unlock_time);
                        unlocked.push((id, unlock_time));
                    }
                }
            }
        }

        println!("  📊 Total unlocked achievements found: {}", unlocked.len());
        Ok(unlocked)
    }

    /// Parse Goldberg achievements file for unlocks
    fn parse_goldberg_unlocks(
        file_path: &PathBuf,
        _db_map: &HashMap<String, Achievement>,
    ) -> Result<Vec<(String, i64)>, String> {
        let contents = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read Goldberg file: {}", e))?;

        let achievements: HashMap<String, serde_json::Value> = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;

        let mut unlocked = Vec::new();

        for (ach_id, ach_data) in achievements {
            let earned = ach_data.get("earned")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if earned {
                let earned_time = ach_data.get("earned_time")
                    .and_then(|v| v.as_i64())
                    .filter(|&t| t > 0)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp());

                unlocked.push((ach_id, earned_time));
            }
        }

        Ok(unlocked)
    }

    /// Periodic check for games without sources (every 10 minutes)
    pub async fn check_pending_games(&self) {
        let now = SystemTime::now();

        // Collect games to check in a separate block
        let to_check = {
            let pending = self.pending_games.lock().unwrap();
            let mut to_check = Vec::new();

            for (app_id, (game_name, last_check)) in pending.iter() {
                if let Ok(duration) = now.duration_since(*last_check) {
                    if duration.as_secs() >= 600 {  // 10 minutes
                        to_check.push((*app_id, game_name.clone()));
                    }
                }
            }

            to_check
        }; // Lock is dropped here

        for (app_id, game_name) in to_check {
            println!("  🔄 Checking for achievement source for {} (periodic check)...", game_name);

            if let Some(source) = self.find_achievement_source(app_id, &game_name) {
                // Found a source!
                println!("  ✓ Found source for {}!", game_name);
                self.setup_file_watcher(source.clone(), self.steam_client.clone()).await;

                // Move from pending to watched
                {
                    let mut pending = self.pending_games.lock().unwrap();
                    pending.remove(&app_id);
                }

                {
                    let mut watched = self.watched_games.lock().unwrap();
                    watched.insert(app_id, source);
                }
            } else {
                // Still not found, update last check time
                let mut pending = self.pending_games.lock().unwrap();
                if let Some((_, ref mut last_check)) = pending.get_mut(&app_id) {
                    *last_check = now;
                }
            }
        }
    }
}
