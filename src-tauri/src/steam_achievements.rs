use steamworks::Client;
use crate::achievements::{Achievement};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use scraper::{Html, Selector};

#[derive(Debug, Deserialize)]
struct SteamApiResponse {
    game: Option<SteamGameSchema>,
}

#[derive(Debug, Deserialize)]
struct SteamGameSchema {
    #[serde(rename = "availableGameStats")]
    available_game_stats: Option<AvailableGameStats>,
}

#[derive(Debug, Deserialize)]
struct AvailableGameStats {
    achievements: Option<Vec<SteamAchievementSchema>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SteamAchievementSchema {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    #[serde(rename = "icongray")]
    pub icon_gray: Option<String>,
    pub hidden: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PlayerAchievementsResponse {
    playerstats: Option<PlayerStats>,
}

#[derive(Debug, Deserialize)]
struct PlayerStats {
    achievements: Option<Vec<PlayerAchievement>>,
}

#[derive(Debug, Deserialize)]
struct PlayerAchievement {
    apiname: String,
    achieved: u32,
    unlocktime: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGameSearchResult {
    pub app_id: u32,
    pub name: String,
    pub header_image: Option<String>,
}

pub struct SteamAchievementClient {
    steam_client: Option<Client>,
    http_client: reqwest::Client,
    api_key: Option<String>,
    steam_id: Option<u64>,
}

impl SteamAchievementClient {
    pub fn new(api_key: Option<String>, steam_id: Option<String>) -> Result<Self, String> {
        // Try to initialize Steamworks client, but don't fail if it's not available
        let steam_client = match Client::init() {
            Ok((client, _single)) => {
                println!("✓ Steamworks SDK initialized successfully");
                Some(client)
            }
            Err(e) => {
                println!("⚠ Steamworks SDK not available: {:?}", e);
                println!("  Will use Steam Web API only");
                None
            }
        };

        let http_client = reqwest::Client::new();

        // Parse Steam ID from config if provided
        let steam_id_u64 = steam_id.and_then(|id| id.parse::<u64>().ok());

        Ok(Self {
            steam_client,
            http_client,
            api_key,
            steam_id: steam_id_u64,
        })
    }

    /// Get achievement schema from Steam Web API
    pub async fn get_achievement_schema(&self, app_id: u32) -> Result<Vec<SteamAchievementSchema>, String> {
        // Check if API key is configured
        let api_key = self.api_key.as_ref()
            .ok_or_else(|| "Steam API key not configured. Please set your API key in Settings.".to_string())?;

        let url = format!(
            "https://api.steampowered.com/ISteamUserStats/GetSchemaForGame/v2/?key={}&appid={}",
            api_key, app_id
        );

        println!("  Fetching from Steam Web API for app_id: {}", app_id);

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch from Steam API: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Steam API returned error: {}", response.status()));
        }

        let api_response: SteamApiResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Steam API response: {}", e))?;

        // Extract achievements from API response
        api_response
            .game
            .and_then(|g| g.available_game_stats)
            .and_then(|s| s.achievements)
            .ok_or_else(|| "No achievements found for this game".to_string())
    }

    /// Parse achievements from Steam Community HTML page using proper HTML parsing
    fn parse_achievements_from_html(&self, html: &str, app_id: u32) -> Result<Vec<SteamAchievementSchema>, String> {
        let document = Html::parse_document(html);
        let mut achievements = Vec::new();

        // Try to find all img tags that contain Steam CDN achievement icons
        // Steam CDN URLs look like: https://cdn.fastly.steamstatic.com/steamcommunity/public/images/apps/{app_id}/...
        let img_selector = Selector::parse("img")
            .map_err(|e| format!("Failed to create img selector: {:?}", e))?;

        for img in document.select(&img_selector) {
            // Check if this is an achievement icon by looking for Steam CDN URL
            if let Some(src) = img.value().attr("src") {
                // Look for achievement icons specifically
                if src.contains("steamcommunity/public/images/apps") && src.contains(&app_id.to_string()) {
                    // Find the parent row by going up the tree
                    let mut current = img.parent();
                    let mut achievement_row = None;

                    // Go up the tree to find a parent that might contain all achievement info
                    for _ in 0..5 {
                        if let Some(node) = current {
                            achievement_row = Some(node);
                            current = node.parent();
                        } else {
                            break;
                        }
                    }

                    if let Some(row) = achievement_row {
                        // Try to extract text content from the row
                        let row_element = scraper::ElementRef::wrap(row);

                        if let Some(elem) = row_element {
                            // Get all text from this element
                            let text_parts: Vec<String> = elem.text().map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();

                            // Usually the first non-empty text is the achievement name
                            let display_name = text_parts.get(0).cloned();
                            // Second might be description
                            let description = text_parts.get(1).cloned();

                            if let Some(name) = display_name {
                                if !name.is_empty() && !name.contains('%') {  // Filter out percentage text
                                    // Extract achievement ID from icon URL
                                    let achievement_id = src
                                        .split('/')
                                        .last()
                                        .and_then(|s| s.split('.').next())
                                        .unwrap_or(&name)
                                        .to_string();

                                    // Generate gray icon URL
                                    let icon_gray = src.replace(".jpg", "_gray.jpg");

                                    achievements.push(SteamAchievementSchema {
                                        name: achievement_id,
                                        display_name: name.clone(),
                                        description,
                                        icon: Some(src.to_string()),
                                        icon_gray: Some(icon_gray),
                                        hidden: Some(0),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if achievements.is_empty() {
            println!("  DEBUG: No achievement icons found");
            println!("  DEBUG: HTML length: {} bytes", html.len());
            println!("  DEBUG: Looking for app_id {} in icon URLs", app_id);

            // Debug: Print first 1000 characters to see what we got
            if html.len() > 0 {
                println!("  DEBUG: HTML preview: {}", &html[..html.len().min(1000)]);
            }

            Err("No achievements found for this game".to_string())
        } else {
            println!("  ✓ Successfully parsed {} achievements", achievements.len());
            // Debug: Print first achievement's icon URL
            if let Some(first) = achievements.first() {
                println!("  ✓ First achievement: {}", first.display_name);
                println!("  ✓ First achievement icon: {}", first.icon.as_ref().unwrap_or(&"None".to_string()));
                println!("  ✓ First achievement icon_gray: {}", first.icon_gray.as_ref().unwrap_or(&"None".to_string()));
            }
            Ok(achievements)
        }
    }

    /// Get global achievement percentages from Steam Web API
    async fn get_global_achievement_percentages(&self, app_id: u32) -> Result<std::collections::HashMap<String, f32>, String> {
        let url = format!(
            "https://api.steampowered.com/ISteamUserStats/GetGlobalAchievementPercentagesForApp/v2/?gameid={}",
            app_id
        );

        println!("  Fetching global achievement percentages for app_id: {}", app_id);

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch global percentages: {}", e))?;

        #[derive(Debug, Deserialize)]
        struct GlobalPercentagesResponse {
            achievementpercentages: Option<GlobalPercentagesData>,
        }

        #[derive(Debug, Deserialize)]
        struct GlobalPercentagesData {
            achievements: Option<Vec<GlobalAchievementPercentage>>,
        }

        #[derive(Debug, Deserialize)]
        struct GlobalAchievementPercentage {
            name: String,
            percent: f32,
        }

        let percentages_response: GlobalPercentagesResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse global percentages: {}", e))?;

        let mut result = std::collections::HashMap::new();

        if let Some(data) = percentages_response.achievementpercentages {
            if let Some(achievements) = data.achievements {
                for ach in achievements {
                    result.insert(ach.name, ach.percent);
                }
                println!("  ✓ Loaded global percentages for {} achievements", result.len());
            }
        }

        Ok(result)
    }

    /// Get player's achievement progress from Steam Web API
    /// This requires knowing the user's Steam ID, which we can get from the Steamworks SDK
    async fn get_player_achievements(&self, app_id: u32, steam_id: u64) -> Result<Vec<PlayerAchievement>, String> {
        // Build URL with optional API key
        let url = if let Some(ref api_key) = self.api_key {
            format!(
                "https://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v1/?appid={}&steamid={}&key={}",
                app_id, steam_id, api_key
            )
        } else {
            format!(
                "https://api.steampowered.com/ISteamUserStats/GetPlayerAchievements/v1/?appid={}&steamid={}",
                app_id, steam_id
            )
        };

        println!("  Requesting: {}", url.replace(self.api_key.as_ref().unwrap_or(&String::new()), "***"));

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch player achievements: {}", e))?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        println!("  Response status: {}", status);
        println!("  Response preview: {}", &response_text[..response_text.len().min(200)]);

        let api_response: PlayerAchievementsResponse = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse player achievements: {} - Response: {}", e, response_text))?;

        api_response
            .playerstats
            .and_then(|s| s.achievements)
            .ok_or_else(|| "No achievement data found for this player/game".to_string())
    }

    /// Search for Steam games by name
    pub async fn search_games(&self, query: &str) -> Result<Vec<SteamGameSearchResult>, String> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Use the Steam Store API to search for games
        let url = format!(
            "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
            urlencoding::encode(query)
        );

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to search games: {}", e))?;

        #[derive(Deserialize)]
        struct StoreSearchResponse {
            items: Option<Vec<StoreSearchItem>>,
        }

        #[derive(Deserialize)]
        struct StoreSearchItem {
            id: u32,
            name: String,
            #[serde(rename = "type")]
            item_type: String,
            tiny_image: Option<String>,
        }

        let search_response: StoreSearchResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse search results: {}", e))?;

        let results = search_response
            .items
            .unwrap_or_default()
            .into_iter()
            .filter(|item| item.item_type == "app" || item.item_type == "game")
            .take(20)
            .map(|item| SteamGameSearchResult {
                app_id: item.id,
                name: item.name,
                header_image: item.tiny_image,
            })
            .collect();

        Ok(results)
    }

    /// Scan achievements for a game using hybrid approach
    /// Returns a vector of achievements to be inserted by the caller
    pub async fn scan_achievements_for_game(&self, app_id: u32, game_name: &str) -> Result<Vec<Achievement>, String> {
        println!("  Fetching achievement schema for {}...", game_name);

        // Get achievement schema
        let schema = self.get_achievement_schema(app_id).await?;

        if schema.is_empty() {
            return Ok(Vec::new());
        }

        println!("  Found {} achievements in schema", schema.len());

        // Get global achievement percentages
        let global_percentages = self.get_global_achievement_percentages(app_id).await.ok();

        // Try to get player's Steam ID
        // Priority 1: Use Steam ID from config
        // Priority 2: Use Steamworks SDK to get Steam ID
        let steam_id = self.steam_id.or_else(|| {
            if let Some(ref client) = self.steam_client {
                Some(client.user().steam_id().raw())
            } else {
                None
            }
        });

        // Get player's achievement progress if we have their Steam ID
        let player_achievements = if let Some(sid) = steam_id {
            println!("  Fetching unlock status for Steam ID {}...", sid);
            match self.get_player_achievements(app_id, sid).await {
                Ok(achs) => {
                    println!("  ✓ Successfully fetched unlock status for {} achievements", achs.len());
                    Some(achs)
                }
                Err(e) => {
                    println!("  ⚠ Failed to fetch player achievements: {}", e);
                    println!("    Possible reasons:");
                    println!("    - Your Steam profile is private (set it to Public in Steam Privacy Settings)");
                    println!("    - You don't own this game on this Steam account");
                    println!("    - The game doesn't have achievements API enabled");
                    None
                }
            }
        } else {
            println!("  ⚠ No Steam ID available - achievements will show as locked");
            println!("    Configure your Steam ID in Settings to see your unlock status");
            None
        };

        // Combine schema with player progress
        let now = Utc::now().timestamp();
        let mut achievements = Vec::new();

        for (index, ach_schema) in schema.iter().enumerate() {
            // Find unlock status for this achievement
            let unlock_info = player_achievements.as_ref().and_then(|achs| {
                achs.iter().find(|a| a.apiname == ach_schema.name)
            });

            // Get global unlock percentage for this achievement
            let global_percentage = global_percentages.as_ref()
                .and_then(|percentages| percentages.get(&ach_schema.name))
                .copied();

            let achievement = Achievement {
                id: None,
                app_id,
                game_name: game_name.to_string(),
                achievement_id: ach_schema.name.clone(),
                display_name: ach_schema.display_name.clone(),
                description: ach_schema.description.clone().unwrap_or_default(),
                icon_url: ach_schema.icon.clone(),
                icon_gray_url: ach_schema.icon_gray.clone(),
                hidden: ach_schema.hidden.unwrap_or(0) == 1,
                achieved: unlock_info.map(|u| u.achieved == 1).unwrap_or(false),
                unlock_time: unlock_info.and_then(|u| u.unlocktime),
                source: "Steam".to_string(),
                last_updated: now,
                global_unlock_percentage: global_percentage,
            };

            // Debug: Print first achievement being saved
            if index == 0 {
                println!("  DEBUG: Saving first achievement with icon_url: {:?}", achievement.icon_url);
                println!("  DEBUG: Saving first achievement with icon_gray_url: {:?}", achievement.icon_gray_url);
            }

            achievements.push(achievement);
        }

        Ok(achievements)
    }
}
