// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod steam_monitor;
mod process_monitor;
mod ludusavi;
mod notifications;

use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayEvent, Manager, State, Window};
use tauri::api::dialog;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use config::{ConfigManager, AppConfig};
use steam_monitor::SteamMonitor;
use process_monitor::ProcessMonitor;
use ludusavi::LudusaviManager;
use notifications::NotificationManager;

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<ConfigManager>>,
    steam_handle: Arc<Mutex<Option<mpsc::Sender<MonitorCommand>>>>,
    process_handle: Arc<Mutex<Option<mpsc::Sender<bool>>>>,
    notification_manager: Arc<NotificationManager>,
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
        .add_filter("Executables", &["exe"])
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
                    state.notification_manager.show_backup_success(
                        &game_name,
                        result.files_backed_up.unwrap_or(0),
                        &result.total_size.unwrap_or_default(),
                    );
                }
            } else if result.not_found.unwrap_or(false) {
                if notifications_enabled {
                    state.notification_manager.show_game_not_found(&game_name);
                }
                
                // Send to frontend
                let _ = app_handle.emit_all("game-not-found", serde_json::json!({ "name": game_name }));
            } else {
                if notifications_enabled {
                    state.notification_manager.show_backup_failed(
                        &game_name,
                        &result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Backup error: {}", e);
            if notifications_enabled {
                state.notification_manager.show_error("Backup Error", &format!("Error backing up {}", game_name));
            }
        }
    }
}

async fn start_monitors(state: &AppState, window: Window) {
    let config = {
        let cfg = state.config.lock().unwrap();
        cfg.get_all()
    };
    
    if config.ludusavi_path.is_empty() || config.backup_path.is_empty() {
        println!("Configuration incomplete, skipping monitor initialization");
        return;
    }
    
    let app_handle = window.app_handle();
    
    // Start Steam monitor first
    if !config.steam_api_key.is_empty() && !config.steam_user_id.is_empty() {
        let (tx, mut rx) = mpsc::channel(10);
        let api_key = config.steam_api_key.clone();
        let user_id = config.steam_user_id.clone();
        let state_clone = state.clone();
        let app_clone = app_handle.clone();
        
        tokio::spawn(async move {
            let mut monitor = SteamMonitor::new(api_key, user_id);
            let mut paused = false;
            
            loop {
                tokio::select! {
                    // Check for commands
                    Some(cmd) = rx.recv() => {
                        match cmd {
                            MonitorCommand::Stop => {
                                println!("Steam monitor stopped");
                                break;
                            }
                            MonitorCommand::Pause => {
                                println!("Steam monitor paused");
                                paused = true;
                            }
                            MonitorCommand::Resume => {
                                println!("Steam monitor resumed");
                                paused = false;
                            }
                        }
                    }
                    // Check Steam if not paused
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                        if !paused {
                            if let Some(event) = monitor.check_steam().await {
                                match event {
                                    steam_monitor::GameEvent::Ended(game) => {
                                        println!("Steam game ended: {}", game.name);
                                        handle_game_backup(game.name, &state_clone, app_clone.clone()).await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        });
        
        *state.steam_handle.lock().unwrap() = Some(tx);
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
                                        state_clone.notification_manager.show_game_detected(&game.name);
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
                                        state_clone.notification_manager.show_game_ended(&game.name);
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
    }
}

async fn stop_monitors(state: &AppState) {
    // Stop Steam monitor
    let steam_tx = state.steam_handle.lock().unwrap().take();
    if let Some(tx) = steam_tx {
        let _ = tx.send(MonitorCommand::Stop).await;
    }
    
    // Stop process monitor
    let process_tx = state.process_handle.lock().unwrap().take();
    if let Some(tx) = process_tx {
        let _ = tx.send(true).await;
    }
    
    // Give monitors time to shut down gracefully
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
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
    tauri::Builder::default()
        .setup(|app| {
            let config = Arc::new(Mutex::new(ConfigManager::new()));
            let notification_manager = Arc::new(NotificationManager::new());
            
            let state = AppState {
                config: config.clone(),
                steam_handle: Arc::new(Mutex::new(None)),
                process_handle: Arc::new(Mutex::new(None)),
                notification_manager,
            };
            
            app.manage(state.clone());
            
            // Initialize monitors
            let window = app.get_window("main").unwrap();
            let state_clone = state.clone();
            tauri::async_runtime::spawn(async move {
                start_monitors(&state_clone, window).await;
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
            get_ludusavi_manifest
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}