use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub name: String,
    pub exe_path: String,
}

pub enum GameEvent {
    Started(GameInfo),
    Ended(GameInfo),
}

pub struct ProcessMonitor {
    game_executables: HashMap<String, String>, // game_name -> exe_path
    current_games: HashSet<String>,
    system: System,
}

impl ProcessMonitor {
    pub fn new(game_executables: HashMap<String, String>) -> Self {
        Self {
            game_executables,
            current_games: HashSet::new(),
            system: System::new_all(),
        }
    }
    
    pub async fn check_processes(&mut self) -> Option<GameEvent> {
        self.system.refresh_processes();

        let mut running_games = HashSet::new();

        // Debug: print currently tracked games
        if !self.current_games.is_empty() {
            println!("[ProcessMonitor] Currently tracking: {:?}", self.current_games);
        }
        
        // Check each configured game
        for (game_name, exe_path) in &self.game_executables {
            let exe_name = Path::new(exe_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            
            // Check if this game's executable is running
            // Look for either exact path match or just the exe name match
            let is_running = self.system.processes().iter().any(|(_, process)| {
                let process_name = process.name().to_lowercase();
                let process_exe = process.exe()
                    .and_then(|p| p.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                
                // Match either by:
                // 1. Exact exe name (for generic names like game.exe)
                // 2. Full path match (for precise identification)
                // 3. End of path match (in case path doesn't match exactly but exe location does)
                process_name == exe_name || 
                process_exe == exe_path.to_lowercase() ||
                process_exe.ends_with(&format!("\\{}", exe_name))
            });
            
            if is_running {
                running_games.insert(game_name.clone());
                
                // If this is a newly detected game
                if !self.current_games.contains(game_name) {
                    println!("Game detected: {}", game_name);
                    let event = GameEvent::Started(GameInfo {
                        name: game_name.clone(),
                        exe_path: exe_path.clone(),
                    });
                    self.current_games.insert(game_name.clone());
                    return Some(event);
                }
            }
        }
        
        // Check for games that have ended
        for game_name in self.current_games.clone() {
            if !running_games.contains(&game_name) {
                println!("[ProcessMonitor] Game ended: {}", game_name);
                let exe_path = self.game_executables.get(&game_name)
                    .cloned()
                    .unwrap_or_default();
                
                self.current_games.remove(&game_name);
                return Some(GameEvent::Ended(GameInfo {
                    name: game_name,
                    exe_path,
                }));
            }
        }
        
        None
    }
}