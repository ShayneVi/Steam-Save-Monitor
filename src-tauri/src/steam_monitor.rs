use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use regex::Regex;
use sysinfo::System;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub app_id: u32,
    pub name: String,
}

pub enum GameEvent {
    Started(GameInfo),
    Ended(GameInfo),
}

pub struct SteamMonitor {
    steam_path: PathBuf,
    current_game: Option<GameInfo>,
    last_running_appid: Option<u32>,
    system: System,
    game_executables: HashMap<String, (u32, String)>, // exe_name -> (app_id, game_name)
}

impl SteamMonitor {
    pub fn new() -> Result<Self, String> {
        let steam_path = Self::find_steam_path()?;
        println!("✓ Steam path detected: {}", steam_path.display());

        let mut monitor = Self {
            steam_path: steam_path.clone(),
            current_game: None,
            last_running_appid: None,
            system: System::new_all(),
            game_executables: HashMap::new(),
        };

        // Build game executable map
        monitor.load_steam_games();

        Ok(monitor)
    }

    fn load_steam_games(&mut self) {
        println!("Scanning Steam libraries for installed games...");

        // Get all Steam library folders
        let library_folders = self.get_library_folders();

        for library_path in library_folders {
            let steamapps_path = library_path.join("steamapps");
            if !steamapps_path.exists() {
                continue;
            }

            // Read all appmanifest files
            if let Ok(entries) = fs::read_dir(&steamapps_path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        if filename_str.starts_with("appmanifest_") && filename_str.ends_with(".acf") {
                            self.parse_appmanifest(&path, &steamapps_path);
                        }
                    }
                }
            }
        }

        println!("✓ Loaded {} Steam games for automatic detection", self.game_executables.len());

        // Debug: Show some games
        let mut games: Vec<_> = self.game_executables.iter().take(5).collect();
        games.sort_by_key(|(exe, _)| exe.to_lowercase());
        for (exe, (app_id, name)) in games {
            println!("  - {} -> {} (AppID: {})", exe, name, app_id);
        }
        if self.game_executables.len() > 5 {
            println!("  ... and {} more", self.game_executables.len() - 5);
        }
    }

    fn get_library_folders(&self) -> Vec<PathBuf> {
        let mut folders = vec![self.steam_path.clone()];

        let libraryfolders_path = self.steam_path.join("steamapps").join("libraryfolders.vdf");
        if let Ok(contents) = fs::read_to_string(&libraryfolders_path) {
            // Parse library paths using regex
            if let Ok(re) = Regex::new(r#""path"\s+"([^"]+)""#) {
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

        folders
    }

    fn parse_appmanifest(&mut self, manifest_path: &PathBuf, steamapps_path: &PathBuf) {
        if let Ok(contents) = fs::read_to_string(manifest_path) {
            // Extract app ID, name, and install directory
            let app_id_re = match Regex::new(r#""appid"\s+"(\d+)""#) {
                Ok(re) => re,
                Err(_) => return,
            };
            let name_re = match Regex::new(r#""name"\s+"([^"]+)""#) {
                Ok(re) => re,
                Err(_) => return,
            };
            let installdir_re = match Regex::new(r#""installdir"\s+"([^"]+)""#) {
                Ok(re) => re,
                Err(_) => return,
            };

            let app_id = match app_id_re.captures(&contents)
                .and_then(|cap| cap.get(1))
                .and_then(|m| m.as_str().parse::<u32>().ok()) {
                Some(id) => id,
                None => return,
            };

            let name = match name_re.captures(&contents)
                .and_then(|cap| cap.get(1))
                .map(|m| m.as_str().to_string()) {
                Some(n) => n,
                None => return,
            };

            let installdir = match installdir_re.captures(&contents)
                .and_then(|cap| cap.get(1))
                .map(|m| m.as_str().to_string()) {
                Some(dir) => dir,
                None => return,
            };

            // Find executables in the game directory
            let game_path = steamapps_path.join("common").join(&installdir);
            if game_path.exists() {
                self.scan_game_executables(&game_path, app_id, &name);
            }
        }
    }

    fn scan_game_executables(&mut self, game_path: &PathBuf, app_id: u32, game_name: &str) {
        // Recursively search for .exe files (up to 3 levels deep to avoid going too deep)
        self.scan_directory_for_exes(game_path, app_id, game_name, 0, 3);
    }

    fn scan_directory_for_exes(&mut self, dir: &PathBuf, app_id: u32, game_name: &str, depth: usize, max_depth: usize) {
        if depth > max_depth {
            return;
        }

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();

                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext.eq_ignore_ascii_case("exe") {
                            if let Some(filename) = path.file_name() {
                                let exe_name = filename.to_string_lossy().to_string();
                                // Skip common launchers and tools
                                let lower = exe_name.to_lowercase();
                                if !lower.contains("unins") &&
                                   !lower.contains("crash") &&
                                   !lower.contains("report") &&
                                   !lower.contains("setup") &&
                                   !lower.contains("launcher") &&
                                   !lower.contains("redist") {
                                    self.game_executables.insert(
                                        exe_name.clone(),
                                        (app_id, game_name.to_string())
                                    );
                                }
                            }
                        }
                    }
                } else if path.is_dir() && depth < max_depth {
                    self.scan_directory_for_exes(&path, app_id, game_name, depth + 1, max_depth);
                }
            }
        }
    }

    fn find_steam_path() -> Result<PathBuf, String> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;

            let output = Command::new("reg")
                .args(&[
                    "query",
                    "HKEY_CURRENT_USER\\Software\\Valve\\Steam",
                    "/v",
                    "SteamPath",
                ])
                .output();

            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("SteamPath") {
                        if let Some(path) = line.split("REG_SZ").nth(1) {
                            let path = path.trim().replace("/", "\\");
                            return Ok(PathBuf::from(path));
                        }
                    }
                }
            }
        }

        let common_paths = vec![
            r"C:\Program Files (x86)\Steam",
            r"C:\Program Files\Steam",
        ];

        for path_str in common_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Ok(path);
            }
        }

        Err("Steam installation not found".to_string())
    }

    fn get_localconfig_path(&self) -> Result<PathBuf, String> {
        let userdata_path = self.steam_path.join("userdata");

        if !userdata_path.exists() {
            return Err("Steam userdata folder not found".to_string());
        }

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

        let user_dir = &user_dirs[0];
        let localconfig = user_dir.path().join("config").join("localconfig.vdf");

        if !localconfig.exists() {
            return Err(format!(
                "localconfig.vdf not found at: {}",
                localconfig.display()
            ));
        }

        Ok(localconfig)
    }

    fn get_running_game(&mut self) -> Option<GameInfo> {
        // Refresh process list
        self.system.refresh_processes_specifics(sysinfo::ProcessRefreshKind::new());

        // Check all running processes
        for (_pid, process) in self.system.processes() {
            let process_name = process.name();

            // Check if this process matches any of our known Steam games
            if let Some((app_id, game_name)) = self.game_executables.get(process_name) {
                // Exclude Borderless Gaming (AppID 388080) from monitoring
                if *app_id == 388080 {
                    continue;
                }

                return Some(GameInfo {
                    app_id: *app_id,
                    name: game_name.clone(),
                });
            }
        }

        None
    }

    fn get_game_name(&self, app_id: u32) -> String {
        let steamapps_path = self.steam_path.join("steamapps");

        if steamapps_path.exists() {
            let manifest_path = steamapps_path.join(format!("appmanifest_{}.acf", app_id));

            if manifest_path.exists() {
                if let Ok(contents) = fs::read_to_string(&manifest_path) {
                    // Simple regex to find "name"\t"Game Name"
                    if let Ok(re) = Regex::new(r#""name"\s*"([^"]+)""#) {
                        if let Some(captures) = re.captures(&contents) {
                            if let Some(name) = captures.get(1) {
                                return name.as_str().to_string();
                            }
                        }
                    }
                }
            }
        }

        format!("App {}", app_id)
    }

    pub fn check_steam(&mut self) -> Option<GameEvent> {
        let current_running = self.get_running_game();
        let current_appid = current_running.as_ref().map(|g| g.app_id);

        match (&self.last_running_appid, current_appid) {
            (None, Some(app_id)) => {
                if let Some(game) = current_running {
                    println!("Game detected: {} (AppID: {})", game.name, game.app_id);
                    self.last_running_appid = Some(app_id);
                    self.current_game = Some(game.clone());
                    Some(GameEvent::Started(game))
                } else {
                    None
                }
            }
            (Some(_old_app_id), None) => {
                if let Some(old_game) = self.current_game.take() {
                    println!("Game ended: {} (AppID: {})", old_game.name, old_game.app_id);
                    self.last_running_appid = None;
                    Some(GameEvent::Ended(old_game))
                } else {
                    self.last_running_appid = None;
                    None
                }
            }
            (Some(old_app_id), Some(new_app_id)) if old_app_id != &new_app_id => {
                if let Some(old_game) = self.current_game.take() {
                    println!(
                        "Game switched: {} -> {}",
                        old_game.name,
                        current_running.as_ref().map(|g| g.name.as_str()).unwrap_or("Unknown")
                    );
                    self.last_running_appid = Some(new_app_id);
                    self.current_game = current_running.clone();
                    Some(GameEvent::Ended(old_game))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn is_steam_running(&self) -> bool {
        let steam_exe = self.steam_path.join("Steam.exe");
        steam_exe.exists()
    }
}

unsafe impl Send for SteamMonitor {}
