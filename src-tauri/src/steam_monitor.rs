use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub app_id: u32,
    pub name: String,
}

pub enum GameEvent {
    Started(GameInfo),
    Ended(GameInfo),
}

#[derive(Debug, Deserialize)]
struct PlayerSummary {
    response: PlayerResponse,
}

#[derive(Debug, Deserialize)]
struct PlayerResponse {
    players: Vec<Player>,
}

#[derive(Debug, Deserialize)]
struct Player {
    gameid: Option<String>,
    gameextrainfo: Option<String>,
}

pub struct SteamMonitor {
    api_key: String,
    steam_user_id: String,
    current_game: Option<GameInfo>,
    system: System,
    client: reqwest::Client,
}

impl SteamMonitor {
    pub fn new(api_key: String, steam_user_id: String) -> Self {
        Self {
            api_key,
            steam_user_id,
            current_game: None,
            system: System::new_all(),
            client: reqwest::Client::new(),
        }
    }
    
    pub async fn check_steam(&mut self) -> Option<GameEvent> {
        // Check if Steam is running
        self.system.refresh_processes();
        let steam_running = self.system.processes().iter().any(|(_, p)| {
            let name = p.name().to_lowercase();
            name.contains("steam") && !name.contains("steamwebhelper")
        });
        
        if !steam_running {
            if let Some(game) = self.current_game.take() {
                return Some(GameEvent::Ended(game));
            }
            return None;
        }
        
        // Get currently playing game from Steam API
        match self.get_current_game().await {
            Ok(Some(currently_playing)) => {
                if let Some(ref current) = self.current_game {
                    if current.app_id != currently_playing.app_id {
                        // Different game - end previous, start new
                        let ended = current.clone();
                        self.current_game = Some(currently_playing.clone());
                        // Return ended event (started will be detected next loop)
                        return Some(GameEvent::Ended(ended));
                    }
                } else {
                    // Game started
                    self.current_game = Some(currently_playing.clone());
                    return Some(GameEvent::Started(currently_playing));
                }
            }
            Ok(None) => {
                if let Some(game) = self.current_game.take() {
                    // Game ended
                    return Some(GameEvent::Ended(game));
                }
            }
            Err(e) => {
                eprintln!("Error checking Steam: {}", e);
            }
        }
        
        None
    }
    
    async fn get_current_game(&self) -> Result<Option<GameInfo>, Box<dyn std::error::Error>> {
        let url = format!(
            "https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v2/?key={}&steamids={}",
            self.api_key, self.steam_user_id
        );
        
        let response = self.client.get(&url).send().await?;
        let data: PlayerSummary = response.json().await?;
        
        if let Some(player) = data.response.players.first() {
            if let Some(gameid) = &player.gameid {
                let app_id = gameid.parse::<u32>().unwrap_or(0);
                let name = player.gameextrainfo.clone().unwrap_or_else(|| "Unknown Game".to_string());
                
                return Ok(Some(GameInfo { app_id, name }));
            }
        }
        
        Ok(None)
    }
}