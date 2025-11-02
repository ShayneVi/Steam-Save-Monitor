use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::achievements::{Achievement, AchievementDatabase};
use chrono::Utc;
use crate::steam_achievements::SteamAchievementClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamAchievement {
    pub achievement: String,
    pub unlocked: i32,
    pub unlocktime: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldbergAchievement {
    pub earned: bool,
    pub earned_time: Option<i64>,
    pub name: String,
    pub description: Option<String>,
}

pub struct AchievementScanner {
    steam_path: PathBuf,
    steam_userdata_path: Option<PathBuf>,
}

impl AchievementScanner {
    pub fn new(steam_path: PathBuf, user_id: Option<String>) -> Result<Self, String> {
        let userdata_path = Self::find_steam_userdata(&steam_path, user_id)?;

        Ok(Self {
            steam_path,
            steam_userdata_path: Some(userdata_path),
        })
    }

    fn find_steam_userdata(steam_path: &PathBuf, user_id: Option<String>) -> Result<PathBuf, String> {
        let userdata_path = steam_path.join("userdata");

        if !userdata_path.exists() {
            return Err("Steam userdata folder not found".to_string());
        }

        // If user ID is provided, use it directly
        if let Some(id) = user_id {
            let user_path = userdata_path.join(&id);
            if user_path.exists() && user_path.is_dir() {
                println!("  Using configured Steam user ID: {}", id);
                return Ok(user_path);
            } else {
                return Err(format!("Steam user ID '{}' not found", id));
            }
        }

        // Otherwise, find the first valid user directory (excluding "0" and "ac")
        let user_dirs: Vec<_> = fs::read_dir(&userdata_path)
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

        let selected_user = user_dirs[0].path();
        if let Some(user_name) = selected_user.file_name() {
            println!("  Auto-detected Steam user ID: {:?} (configure this in Settings if incorrect)", user_name);
        }
        Ok(selected_user)
    }

    /// Scan Steam's official achievement files from librarycache
    pub async fn scan_steam_achievements(&self, app_id: u32, game_name: &str, db_path: PathBuf, steam_client: &SteamAchievementClient) -> Result<usize, String> {
        let Some(ref userdata_path) = self.steam_userdata_path else {
            return Err("Steam userdata path not set".to_string());
        };

        // Try librarycache first (the most up-to-date source)
        let librarycache_path = userdata_path.join("config").join("librarycache").join(format!("{}.json", app_id));
        if librarycache_path.exists() {
            match self.parse_librarycache_achievements(&librarycache_path, app_id, game_name, db_path.clone(), steam_client).await {
                Ok(count) if count > 0 => return Ok(count),
                Ok(_) => {}, // No achievements found, try other sources
                Err(e) => println!("  ⚠ Librarycache parse error: {}", e),
            }
        }

        // Fallback to stats folder (these don't use Steam API schema)
        let stats_path = userdata_path.join("stats").join(format!("{}", app_id));

        // Try achievements.json
        let achievements_json = stats_path.join("achievements.json");
        if achievements_json.exists() {
            if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                return self.parse_steam_achievements_json(&achievements_json, app_id, game_name, &db);
            }
        }

        // Try achievements.vdf as fallback
        let achievements_vdf = stats_path.join("achievements.vdf");
        if achievements_vdf.exists() {
            if let Ok(db) = AchievementDatabase::new(db_path.clone()) {
                return self.parse_steam_achievements_vdf(&achievements_vdf, app_id, game_name, &db);
            }
        }

        Ok(0)
    }

    /// Parse librarycache achievement JSON files
    async fn parse_librarycache_achievements(&self, path: &PathBuf, app_id: u32, game_name: &str, db_path: PathBuf, steam_client: &SteamAchievementClient) -> Result<usize, String> {
        println!("  Found LibraryCache achievements at: {:?}", path);

        // STEP 1: Get achievement schema from Steam Web API to get the full list
        let steam_schema = steam_client.get_achievement_schema(app_id).await?;

        if steam_schema.is_empty() {
            return Err("No achievements found in Steam API schema".to_string());
        }

        println!("  ✓ Retrieved {} achievements from Steam API", steam_schema.len());

        // Get global achievement percentages
        let global_percentages = steam_client.get_global_achievement_percentages(app_id).await.ok();
        if global_percentages.is_some() {
            println!("  ✓ Retrieved global achievement percentages");
        }

        // STEP 2: Read library cache to see which ones are unlocked
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read librarycache file: {}", e))?;

        // Parse the nested JSON array structure
        let json: serde_json::Value = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse librarycache JSON: {}", e))?;

        // Find the "achievements" entry in the array
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

        // STEP 3: Build a map of unlocked achievements from library cache
        let mut unlocked_map: std::collections::HashMap<String, (bool, Option<i64>)> = std::collections::HashMap::new();

        // Process vecHighlight (visible achievements - both achieved and unachieved)
        if let Some(vec_highlight) = achievement_data.get("vecHighlight").and_then(|v| v.as_array()) {
            for ach in vec_highlight {
                if let Some(ach_id) = ach.get("strID").and_then(|v| v.as_str()) {
                    let achieved = ach.get("bAchieved").and_then(|v| v.as_bool()).unwrap_or(false);
                    let unlock_time = ach.get("rtUnlocked").and_then(|v| v.as_i64()).filter(|&t| t > 0);
                    unlocked_map.insert(ach_id.to_string(), (achieved, unlock_time));
                }
            }
        }

        // Process vecUnachieved (remaining unachieved achievements)
        if let Some(vec_unachieved) = achievement_data.get("vecUnachieved").and_then(|v| v.as_array()) {
            for ach in vec_unachieved {
                if let Some(ach_id) = ach.get("strID").and_then(|v| v.as_str()) {
                    unlocked_map.insert(ach_id.to_string(), (false, None));
                }
            }
        }

        // Process vecAchievedHidden (achieved hidden achievements)
        if let Some(vec_achieved_hidden) = achievement_data.get("vecAchievedHidden").and_then(|v| v.as_array()) {
            for ach in vec_achieved_hidden {
                if let Some(ach_id) = ach.get("strID").and_then(|v| v.as_str()) {
                    let unlock_time = ach.get("rtUnlocked").and_then(|v| v.as_i64()).filter(|&t| t > 0);
                    let achieved = ach.get("bAchieved").and_then(|v| v.as_bool()).unwrap_or(true); // Default true for vecAchievedHidden

                    // Only insert/update if this achievement is unlocked OR not already in map
                    if achieved {
                        unlocked_map.insert(ach_id.to_string(), (true, unlock_time));
                    } else if !unlocked_map.contains_key(ach_id) {
                        unlocked_map.insert(ach_id.to_string(), (false, None));
                    }
                }
            }
        }

        // STEP 4: Insert ALL achievements from Steam schema, marking as unlocked based on library cache
        let game_name = game_name.to_string();
        tokio::task::spawn_blocking(move || {
            // Open database connection in the blocking task
            let db = AchievementDatabase::new(db_path)
                .map_err(|e| format!("Failed to open database: {}", e))?;

            let now = Utc::now().timestamp();
            let mut unlocked_count = 0;

            for ach_schema in &steam_schema {
                // Check if this achievement is unlocked in library cache
                let (achieved, unlock_time) = unlocked_map
                    .get(&ach_schema.name)
                    .copied()
                    .unwrap_or((false, None));

                // Get global unlock percentage for this achievement
                let global_percentage = global_percentages.as_ref()
                    .and_then(|percentages| percentages.get(&ach_schema.name))
                    .copied();

                let achievement = Achievement {
                    id: None,
                    app_id,
                    game_name: game_name.clone(),
                    achievement_id: ach_schema.name.clone(),
                    display_name: ach_schema.display_name.clone(),
                    description: ach_schema.description.clone().unwrap_or_default(),
                    icon_url: ach_schema.icon.clone(),
                    icon_gray_url: ach_schema.icon_gray.clone(),
                    hidden: ach_schema.hidden.unwrap_or(0) == 1,
                    achieved,
                    unlock_time,
                    source: "Steamtools".to_string(),
                    last_updated: now,
                    global_unlock_percentage: global_percentage,
                };

                db.insert_or_update_achievement(&achievement)?;

                if achieved {
                    unlocked_count += 1;
                }
            }

            Ok(unlocked_count)
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    fn parse_steam_achievements_json(&self, path: &PathBuf, app_id: u32, game_name: &str, db: &AchievementDatabase) -> Result<usize, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read achievements file: {}", e))?;

        let achievements: Vec<SteamAchievement> = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse achievements JSON: {}", e))?;

        let now = Utc::now().timestamp();
        let mut count = 0;

        for ach in achievements {
            let is_unlocked = ach.unlocked == 1;
            let achievement = Achievement {
                id: None,
                app_id,
                game_name: game_name.to_string(),
                achievement_id: ach.achievement.clone(),
                display_name: ach.achievement.clone(), // Will be enhanced with API data later
                description: String::new(),
                icon_url: None,
                icon_gray_url: None,
                hidden: false,
                achieved: is_unlocked,
                unlock_time: ach.unlocktime,
                source: "Steam".to_string(),
                last_updated: now,
                global_unlock_percentage: None,
            };

            db.insert_or_update_achievement(&achievement)?;
            // Only count unlocked achievements
            if is_unlocked {
                count += 1;
            }
        }

        Ok(count)
    }

    fn parse_steam_achievements_vdf(&self, path: &PathBuf, app_id: u32, game_name: &str, db: &AchievementDatabase) -> Result<usize, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read VDF file: {}", e))?;

        // Simple VDF parsing for achievements
        // Format: "achievement_name" { "unlocked" "1" "unlocktime" "1234567890" }
        let regex_ach = regex::Regex::new(r#""([^"]+)"\s*\{\s*"unlocked"\s*"(\d+)"\s*(?:"unlocktime"\s*"(\d+)")?\s*\}"#)
            .map_err(|e| format!("Failed to create regex: {}", e))?;

        let now = Utc::now().timestamp();
        let mut count = 0;

        for cap in regex_ach.captures_iter(&contents) {
            let achievement_id = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let unlocked = cap.get(2).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or(0);
            let unlock_time = cap.get(3).and_then(|m| m.as_str().parse::<i64>().ok());
            let is_unlocked = unlocked == 1;

            let achievement = Achievement {
                id: None,
                app_id,
                game_name: game_name.to_string(),
                achievement_id: achievement_id.to_string(),
                display_name: achievement_id.to_string(),
                description: String::new(),
                icon_url: None,
                icon_gray_url: None,
                hidden: false,
                achieved: is_unlocked,
                unlock_time,
                source: "Steam".to_string(),
                last_updated: now,
                global_unlock_percentage: None,
            };

            db.insert_or_update_achievement(&achievement)?;
            // Only count unlocked achievements
            if is_unlocked {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Scan Goldberg emulator achievements (GSE Saves format)
    pub async fn scan_goldberg_achievements(&self, app_id: u32, game_name: &str, db_path: PathBuf, steam_client: &SteamAchievementClient) -> Result<usize, String> {
        // GSE (Goldberg Steam Emulator) stores achievements in %APPDATA%/GSE Saves/%APPID%/achievements.json
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "Could not get APPDATA environment variable".to_string())?;

        // Try both GSE Saves and Goldberg SteamEmu Saves paths
        let paths = vec![
            PathBuf::from(&appdata).join("GSE Saves").join(format!("{}", app_id)).join("achievements.json"),
            PathBuf::from(&appdata).join("Goldberg SteamEmu Saves").join(format!("{}", app_id)).join("achievements.json"),
        ];

        let mut goldberg_path = None;
        for path in paths {
            if path.exists() {
                goldberg_path = Some(path);
                break;
            }
        }

        let Some(path) = goldberg_path else {
            return Ok(0);
        };

        println!("  Found Goldberg achievements at: {:?}", path);

        // Get achievement schema from Steam Web API to map API names to display names
        let steam_schema = steam_client.get_achievement_schema(app_id).await?;

        // Create lookup map: API name -> (display_name, description)
        let mut steam_by_api_name: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
        for ach in &steam_schema {
            steam_by_api_name.insert(
                ach.name.clone(),
                (ach.display_name.clone(), ach.description.clone().unwrap_or_default())
            );
        }

        println!("  ✓ Retrieved {} achievements from Steam API", steam_schema.len());

        // Get global achievement percentages
        let global_percentages = steam_client.get_global_achievement_percentages(app_id).await.ok();
        if global_percentages.is_some() {
            println!("  ✓ Retrieved global achievement percentages");
        }

        let contents = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read Goldberg achievements: {}", e))?;

        // Parse JSON - Goldberg format is { "ACH_ID": { "earned": bool, "earned_time": timestamp } }
        let achievements: std::collections::HashMap<String, serde_json::Value> = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse Goldberg JSON: {}", e))?;

        // Move database operations into a blocking task
        let game_name = game_name.to_string();
        tokio::task::spawn_blocking(move || {
            // Open database connection in the blocking task
            let db = AchievementDatabase::new(db_path)
                .map_err(|e| format!("Failed to open database: {}", e))?;

            let now = Utc::now().timestamp();
            let mut count = 0;

            for (ach_id, ach_data) in achievements {
                let earned = ach_data.get("earned")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let earned_time = ach_data.get("earned_time")
                    .and_then(|v| v.as_i64())
                    .filter(|&t| t > 0);

                // Look up display name and description from Steam API
                let (display_name, description) = steam_by_api_name
                    .get(&ach_id)
                    .map(|(name, desc)| (name.clone(), desc.clone()))
                    .unwrap_or_else(|| (ach_id.clone(), String::new()));

                // Get global unlock percentage for this achievement
                let global_percentage = global_percentages.as_ref()
                    .and_then(|percentages| percentages.get(&ach_id))
                    .copied();

                let achievement = Achievement {
                    id: None,
                    app_id,
                    game_name: game_name.clone(),
                    achievement_id: ach_id.clone(),
                    display_name,
                    description,
                    icon_url: None,
                    icon_gray_url: None,
                    hidden: false,
                    achieved: earned,
                    unlock_time: earned_time,
                    source: "Goldberg".to_string(),
                    last_updated: now,
                    global_unlock_percentage: global_percentage,
                };

                db.insert_or_update_achievement(&achievement)?;
                // Only count unlocked achievements
                if earned {
                    count += 1;
                }
            }

            Ok(count)
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    /// Scrape Steam Community page to get achievement schema with API names
    async fn scrape_steam_community_achievements(&self, app_id: u32) -> Result<Vec<(String, String, String)>, String> {
        let url = format!("https://steamcommunity.com/stats/{}/achievements/", app_id);

        let response = reqwest::Client::new()
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Steam Community page: {}", e))?;

        let html = response.text().await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let document = scraper::Html::parse_document(&html);
        let row_selector = scraper::Selector::parse(".achieveRow").unwrap();
        let h3_selector = scraper::Selector::parse("h3").unwrap();
        let h5_selector = scraper::Selector::parse("h5").unwrap();
        let img_selector = scraper::Selector::parse("img").unwrap();

        let mut achievements = Vec::new();

        for row in document.select(&row_selector) {
            let display_name = row.select(&h3_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string());

            let description = row.select(&h5_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string());

            // Try to extract API name from image src (e.g., /images/apps/1623730/achievements/Pal_Achievement_6.jpg)
            let api_name = row.select(&img_selector)
                .next()
                .and_then(|img| img.value().attr("src"))
                .and_then(|src| {
                    src.split('/').last()
                        .and_then(|filename| filename.split('.').next())
                        .map(|s| s.to_string())
                });

            if let Some(name) = display_name {
                if !name.is_empty() {
                    achievements.push((
                        api_name.unwrap_or_default(),
                        name,
                        description.unwrap_or_default()
                    ));
                }
            }
        }

        if achievements.is_empty() {
            Err("No achievements found on Steam Community page".to_string())
        } else {
            println!("  ✓ Scraped {} achievements from Steam Community", achievements.len());
            Ok(achievements)
        }
    }

    /// Scan Online-fix emulator achievements
    pub async fn scan_onlinefix_achievements(&self, app_id: u32, game_name: &str, db_path: PathBuf, steam_client: &SteamAchievementClient) -> Result<usize, String> {
        // Online-fix stores achievements in C:\Users\Public\Documents\OnlineFix\[APPID]\Stats\Achievements.ini
        // Try different case variations for compatibility
        let onlinefix_base = PathBuf::from(r"C:\Users\Public\Documents\OnlineFix")
            .join(format!("{}", app_id));

        let onlinefix_path = if onlinefix_base.join("Stats").join("Achievements.ini").exists() {
            onlinefix_base.join("Stats").join("Achievements.ini")
        } else if onlinefix_base.join("stats").join("Achievements.ini").exists() {
            onlinefix_base.join("stats").join("Achievements.ini")
        } else if onlinefix_base.join("Stats").join("achievements.ini").exists() {
            onlinefix_base.join("Stats").join("achievements.ini")
        } else if onlinefix_base.join("stats").join("achievements.ini").exists() {
            onlinefix_base.join("stats").join("achievements.ini")
        } else {
            return Ok(0);
        };

        println!("  Found Online-fix achievements at: {:?}", onlinefix_path);

        // Get achievement schema from Steam Web API using configured API key
        let steam_schema = steam_client.get_achievement_schema(app_id).await?;

        // Convert schema to tuple format (api_name, display_name, description)
        let steam_achievements: Vec<(String, String, String)> = steam_schema.iter().map(|ach| {
            (
                ach.name.clone(),
                ach.display_name.clone(),
                ach.description.clone().unwrap_or_default()
            )
        }).collect();

        println!("  ✓ Retrieved {} achievements from Steam API", steam_achievements.len());

        // Get global achievement percentages
        let global_percentages = steam_client.get_global_achievement_percentages(app_id).await.ok();
        if global_percentages.is_some() {
            println!("  ✓ Retrieved global achievement percentages");
        }

        let contents = fs::read_to_string(&onlinefix_path)
            .map_err(|e| format!("Failed to read Online-fix INI: {}", e))?;

        // Move all database operations into a blocking task
        let game_name = game_name.to_string();
        tokio::task::spawn_blocking(move || {
            // Open database connection in the blocking task
            let db = AchievementDatabase::new(db_path)
                .map_err(|e| format!("Failed to open database: {}", e))?;

            let now = Utc::now().timestamp();
            let mut count = 0;

            // Create lookup map by API name
            let mut steam_by_api_name: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
            let mut steam_by_index: Vec<(String, String)> = Vec::new();

            for (api_name, display_name, description) in &steam_achievements {
                // Map API name to (display_name, description)
                steam_by_api_name.insert(api_name.clone(), (display_name.clone(), description.clone()));
                steam_by_index.push((display_name.clone(), description.clone()));
            }

            // Parse INI file to find unlocked achievements
            let section_regex = regex::Regex::new(r"(?m)^\[([^\]]+)\]")
                .map_err(|e| format!("Failed to create section regex: {}", e))?;

            let achieved_regex = regex::Regex::new(r"(?m)^achieved\s*=\s*(\w+)")
                .map_err(|e| format!("Failed to create achieved regex: {}", e))?;

            let timestamp_regex = regex::Regex::new(r"(?m)^timestamp\s*=\s*(\d+)")
                .map_err(|e| format!("Failed to create timestamp regex: {}", e))?;

            // Extract trailing number from section name (e.g., "ACH_23" -> 23, "Achievement_Trophy24" -> 24)
            let number_regex = regex::Regex::new(r"(\d+)$")
                .map_err(|e| format!("Failed to create number regex: {}", e))?;

            // Strip common prefixes: ACH_, Achievement_, achievement_, ACHIEVEMENT_
            let prefix_regex = regex::Regex::new(r"^(?i)(ACH_|ACHIEVEMENT_)")
                .map_err(|e| format!("Failed to create prefix regex: {}", e))?;

            // Build a map of unlocked achievements with their unlock times
            let mut unlocked_achievements: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();

            // Parse OnlineFix INI to find unlocked achievements
            for section_cap in section_regex.captures_iter(&contents) {
                let section_match = section_cap.get(0).unwrap();
                let section_name = section_cap.get(1).unwrap().as_str();

                // Find the next section or end of file
                let section_start = section_match.end();
                let next_section_pos = contents[section_start..]
                    .find("\n[")
                    .map(|pos| section_start + pos)
                    .unwrap_or(contents.len());

                let section_content = &contents[section_start..next_section_pos];

                // Extract achieved and timestamp from this section
                let achieved = if let Some(ach_cap) = achieved_regex.captures(section_content) {
                    ach_cap.get(1).map(|m| m.as_str().to_lowercase() == "true").unwrap_or(false)
                } else {
                    false
                };

                // Only process unlocked achievements
                if !achieved {
                    continue;
                }

                let unlock_time = if let Some(ts_cap) = timestamp_regex.captures(section_content) {
                    ts_cap.get(1).and_then(|m| m.as_str().parse::<i64>().ok()).filter(|&t| t > 0).unwrap_or(0)
                } else {
                    0
                };

                // Try to find matching achievement index from Steam:
                // 1. First try exact API name match
                // 2. Then try extracting number and using as index
                // 3. Then try matching by name (after stripping prefixes)
                // 4. Finally try matching by keywords in description
                let ach_index_opt = if let Some((display_name, description)) = steam_by_api_name.get(section_name) {
                    // Exact API name match found!
                    steam_by_index.iter().position(|(name, _)| name == display_name)
                } else if let Some(num_cap) = number_regex.captures(section_name) {
                    // Extract number and use as 1-based index
                    if let Ok(ach_index) = num_cap.get(1).unwrap().as_str().parse::<usize>() {
                        if ach_index > 0 && ach_index <= steam_by_index.len() {
                            Some(ach_index - 1)  // Convert to 0-based
                        } else {
                            println!("  ⚠ {} index {} is out of range (max: {})", section_name, ach_index, steam_by_index.len());
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    // No number found, try matching by name
                    let cleaned_name = prefix_regex.replace(section_name, "").to_string();

                    // Replace underscores with spaces for name matching
                    let name_with_spaces = cleaned_name.replace("_", " ");

                    println!("  DEBUG: Trying name match: '{}' -> '{}'", section_name, name_with_spaces);

                    // Try to match with display name (case-insensitive) and get its index
                    if let Some(idx) = steam_by_index.iter().position(|(name, _)| name.to_lowercase() == name_with_spaces.to_lowercase()) {
                        println!("  ✓ Name matched!");
                        Some(idx)
                    } else {
                        // Name matching failed, try matching by keywords in description
                        // Extract keywords from the achievement ID (e.g., "LoversVengeance10Kills" -> ["lovers", "vengeance", "10", "kills"])

                        // First, split on underscores and other non-alphanumeric chars to get segments
                        let segments: Vec<&str> = cleaned_name
                            .split(|c: char| !c.is_alphanumeric())
                            .filter(|s| !s.is_empty())
                            .collect();

                        println!("  DEBUG: Segments from '{}': {:?}", section_name, segments);

                        let mut all_keywords: Vec<String> = Vec::new();

                        // For each segment, do camelCase splitting and separate numbers
                        for segment in segments {
                            // Check if it's all uppercase (like "FIRST", "TALK")
                            let is_all_caps = segment.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
                            println!("  DEBUG: Segment '{}' is_all_caps={}", segment, is_all_caps);

                            if is_all_caps && segment.len() > 0 {
                                // All caps - treat as single word
                                all_keywords.push(segment.to_lowercase());
                            } else {
                                // Split numbers from letters first (e.g., "kill100" -> "kill", "100")
                                let mut current_word = String::new();
                                let mut last_was_digit = false;

                                for ch in segment.chars() {
                                    let is_digit = ch.is_numeric();

                                    // If transitioning from letter to digit or digit to letter, or uppercase boundary
                                    if !current_word.is_empty() && (
                                        (last_was_digit != is_digit) ||
                                        (ch.is_uppercase() && !last_was_digit)
                                    ) {
                                        all_keywords.push(current_word.to_lowercase());
                                        current_word.clear();
                                    }

                                    current_word.push(ch);
                                    last_was_digit = is_digit;
                                }

                                if !current_word.is_empty() {
                                    all_keywords.push(current_word.to_lowercase());
                                }
                            }
                        }

                        // Filter out short keywords (unless they're numbers)
                        let all_keywords: Vec<String> = all_keywords.into_iter()
                            .filter(|k| k.len() > 2 || k.chars().all(|c| c.is_numeric()))
                            .collect();

                        println!("  DEBUG: Extracted keywords from '{}': {:?}", section_name, all_keywords);

                        if all_keywords.is_empty() {
                            println!("  ⚠ No keywords extracted, skipping keyword matching");
                        }

                        // Helper function to get word root (strip common suffixes)
                        fn get_word_root(word: &str) -> String {
                            let suffixes = ["iac", "ic", "al", "er", "ing", "ed", "ly", "ness", "ment", "ous", "ful"];
                            for suffix in suffixes {
                                if word.len() > suffix.len() + 2 && word.ends_with(suffix) {
                                    return word[..word.len() - suffix.len()].to_string();
                                }
                            }
                            word.to_string()
                        }

                        // Helper function for synonym matching
                        fn is_synonym(word1: &str, word2: &str) -> bool {
                            let synonyms = vec![
                                vec!["boundless", "without", "bounds", "endless", "infinite", "unlimited"],
                                vec!["rage", "anger", "fury", "wrath"],
                                vec!["support", "helper", "assist", "aid"],
                                vec!["specialist", "expert", "master", "main"],
                                vec!["true", "real", "genuine", "authentic"],
                                vec!["kill", "slay", "defeat", "destroy", "eliminate"],
                                vec!["win", "victory", "triumph", "conquer"],
                                vec!["lose", "defeat", "fail", "loss"],
                                vec!["complete", "finish", "done", "accomplish"],
                                vec!["first", "initial", "beginning"],
                            ];

                            for group in synonyms {
                                if group.contains(&word1) && group.contains(&word2) {
                                    return true;
                                }
                            }
                            false
                        }

                        // Helper function for fuzzy character matching
                        fn fuzzy_char_match(word1: &str, word2: &str) -> bool {
                            if word1.len() < 4 || word2.len() < 4 {
                                return false;
                            }
                            let shorter = if word1.len() < word2.len() { word1 } else { word2 };
                            let longer = if word1.len() < word2.len() { word2 } else { word1 };

                            // Count matching characters
                            let mut matches = 0;
                            for ch in shorter.chars() {
                                if longer.contains(ch) {
                                    matches += 1;
                                }
                            }

                            // Require 70% character overlap
                            matches as f32 / shorter.len() as f32 >= 0.7
                        }

                        // Find achievement where description contains all keywords
                        println!("  Searching through {} Steam achievements for match...", steam_by_index.len());
                        let result_position = steam_by_index.iter().enumerate().position(|(idx, (name, desc))| {
                            let desc_lower = desc.to_lowercase().replace("_", " ");
                            let name_lower = name.to_lowercase().replace("_", " ");
                            let combined = format!("{} {}", name_lower, desc_lower);

                            // Count how many keywords match (with enhanced fuzzy matching)
                            let matches = all_keywords.iter()
                                .filter(|kw| {
                                    // Exact match
                                    if combined.contains(kw.as_str()) {
                                        return true;
                                    }

                                    let kw_root = get_word_root(kw);

                                    // Check against all words in the combined string
                                    combined.split(|c: char| !c.is_alphanumeric()).any(|word| {
                                        let word = word.trim();
                                        if word.is_empty() || kw.is_empty() {
                                            return false;
                                        }

                                        // 1. Exact substring match
                                        if kw.contains(word) || word.contains(kw.as_str()) {
                                            return true;
                                        }

                                        // 2. Root word matching (pyroman matches pyromaniac)
                                        let word_root = get_word_root(word);
                                        if kw_root.len() >= 4 && word_root.len() >= 4 {
                                            if kw_root == word_root || kw_root.contains(&word_root) || word_root.contains(&kw_root) {
                                                return true;
                                            }
                                        }

                                        // 3. Synonym matching
                                        if is_synonym(kw, word) {
                                            return true;
                                        }

                                        // 4. Fuzzy character matching (70% overlap)
                                        if fuzzy_char_match(kw, word) {
                                            return true;
                                        }

                                        // 5. Plural/possessive matching
                                        if (kw.len() >= 4 && word.len() >= 4) {
                                            let kw_chars: Vec<char> = kw.chars().collect();
                                            let word_chars: Vec<char> = word.chars().collect();
                                            if kw_chars.len() >= 4 && word_chars.len() >= 4 &&
                                               kw_chars[..kw_chars.len()-1] == word_chars[..word_chars.len()-1] {
                                                return true;
                                            }
                                        }

                                        false
                                    })
                                })
                                .count();

                            let threshold = (all_keywords.len() / 2).max(1);
                            let is_match = !all_keywords.is_empty() && matches >= threshold;

                            // Debug output for first 3 and any matches
                            if idx < 3 || is_match {
                                println!("    [{}] '{}' / '{}': {}/{} keywords matched (threshold: {}) -> {}",
                                    idx, name_lower, desc_lower, matches, all_keywords.len(), threshold,
                                    if is_match { "✓ MATCH" } else { "✗" });
                            }

                            if is_match {
                                println!("  ✓ Found match at index {}: '{}'", idx, name_lower);
                            }

                            is_match
                        });

                        if result_position.is_none() {
                            println!("  ⚠ No match found after testing all {} achievements", steam_by_index.len());
                        }

                        result_position
                    }
                };

                if let Some(idx) = ach_index_opt {
                    unlocked_achievements.insert(idx, unlock_time);
                } else {
                    println!("  ⚠ Could not match achievement: {}", section_name);
                }
            }

            // Now insert ALL achievements from Steam Community
            let mut unlocked_count = 0;
            for (index, (api_name, display_name, description)) in steam_achievements.iter().enumerate() {
                let is_unlocked = unlocked_achievements.contains_key(&index);
                let unlock_time = unlocked_achievements.get(&index).copied().filter(|&t| t > 0);

                // Get global unlock percentage for this achievement
                let global_percentage = global_percentages.as_ref()
                    .and_then(|percentages| percentages.get(api_name))
                    .copied();

                let achievement = Achievement {
                    id: None,
                    app_id,
                    game_name: game_name.clone(),
                    achievement_id: api_name.clone(),  // Use actual Steam API name, not generated ID
                    display_name: display_name.clone(),
                    description: description.clone(),
                    icon_url: None,
                    icon_gray_url: None,
                    hidden: false,
                    achieved: is_unlocked,
                    unlock_time,
                    source: "Online-fix".to_string(),
                    last_updated: now,
                    global_unlock_percentage: global_percentage,
                };

                db.insert_or_update_achievement(&achievement)?;
                count += 1; // Total count
                if is_unlocked {
                    unlocked_count += 1; // Only count unlocked
                }
            }

            Ok(unlocked_count) // Return unlocked count, not total count
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?
    }

    /// Scan all achievement sources for a specific game
    /// Note: All scanning now requires async and is called separately from main.rs
    pub fn scan_all_sources(&self, app_id: u32, game_name: &str, db: &AchievementDatabase) -> Result<usize, String> {
        // This method is deprecated - all scanning is now done async in main.rs
        println!("  ℹ All scanning now requires async context, use add_game_to_tracking instead");
        Ok(0)
    }
}
