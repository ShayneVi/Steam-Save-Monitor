use windows::Win32::Media::Audio::{PlaySoundA, SND_ALIAS, SND_ASYNC};
use windows::core::PCSTR;
use std::ffi::CString;
use std::thread;
use notify_rust::Notification;
use crate::overlay::OverlayManager;
use std::sync::{Arc, Mutex};

pub struct NotificationManager {
    overlay_manager: Option<Arc<Mutex<OverlayManager>>>,
    achievement_duration: Arc<Mutex<u32>>,
}

impl NotificationManager {
    pub fn new(achievement_duration: Arc<Mutex<u32>>) -> Self {
        Self {
            overlay_manager: None,
            achievement_duration,
        }
    }

    pub fn set_overlay_manager(&mut self, overlay_manager: Arc<Mutex<OverlayManager>>) {
        self.overlay_manager = Some(overlay_manager);
    }

    fn play_notification_sound() {
        thread::spawn(move || {
            unsafe {
                // Play Windows default notification sound
                let sound_alias = CString::new("SystemNotification").unwrap_or_default();
                let _ = PlaySoundA(
                    PCSTR(sound_alias.as_ptr() as *const u8),
                    None,
                    SND_ALIAS | SND_ASYNC,
                );
            }
        });
    }

    fn show_notification(&self, title: &str, body: &str) {
        Self::play_notification_sound();
        
        let title = title.to_string();
        let body = body.to_string();
        
        thread::spawn(move || {
            let _ = Notification::new()
                .summary(&title)
                .body(&body)
                .timeout(2500)
                .show();
        });
    }

    pub fn show_backup_success(&self, game_name: &str, files_backed_up: usize, total_size: &str) {
        let body = format!("✓ {} files backed up\nSize: {}", files_backed_up, total_size);
        self.show_notification("Game Save Monitor", &format!("{}\n{}", game_name, body));
    }

    pub fn show_backup_success_with_achievements(&self, game_name: &str, files_backed_up: usize, total_size: &str, achievements_count: usize) {
        let body = if achievements_count > 0 {
            format!("✓ {} files backed up\nSize: {}\n🏆 {} achievements backed up", files_backed_up, total_size, achievements_count)
        } else {
            format!("✓ {} files backed up\nSize: {}", files_backed_up, total_size)
        };
        self.show_notification("Game Save Monitor", &format!("{}\n{}", game_name, body));
    }

    pub fn show_game_detected(&self, game_name: &str) {
        self.show_notification("Game Save Monitor", &format!("{}\n▶ Game Detected - Monitoring saves & achievements...", game_name));
    }

    pub fn show_game_ended(&self, game_name: &str) {
        let game_name = game_name.to_string();
        
        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(300));
            
            let _ = Notification::new()
                .summary("Game Save Monitor")
                .body(&format!("{}\n⏹ Game Ended - Preparing backup...", game_name))
                .timeout(2500)
                .show();
        });
    }

    pub fn show_backup_failed(&self, game_name: &str, error: &str) {
        let body = format!("✗ Backup Failed\nError: {}", error);
        self.show_notification("Game Save Monitor", &format!("{}\n{}", game_name, body));
    }

    pub fn show_game_not_found(&self, game_name: &str) {
        self.show_notification("Game Save Monitor", &format!("{}\n⚠ Not found in Ludusavi\nAdd in Games tab", game_name));
    }

    pub fn show_error(&self, title: &str, message: &str) {
        let body = format!("⚠ {}", message);
        self.show_notification("Game Save Monitor", &format!("{}\n{}", title, body));
    }

    pub fn show_achievement_unlock(&self, game_name: &str, achievement_name: &str, description: &str, icon_url: Option<&str>, global_unlock_percentage: Option<f32>) {
        // Get current duration from state
        let duration_seconds = *self.achievement_duration.lock().unwrap();

        // Try to use overlay if available
        if let Some(overlay_manager) = &self.overlay_manager {
            if let Ok(overlay) = overlay_manager.lock() {
                let notification_data = serde_json::json!({
                    "game_name": game_name,
                    "achievement_name": achievement_name,
                    "achievement_description": description,
                    "icon_url": icon_url,
                    "global_unlock_percentage": global_unlock_percentage,
                    "duration_seconds": duration_seconds
                });

                println!("[NotificationManager] Sending notification with duration: {} seconds", duration_seconds);

                // Try to show on overlay
                if overlay.show_overlay("achievement", notification_data).is_ok() {
                    // Don't play sound here - overlay will handle it based on rarity settings
                    return; // Success! Don't fall back to native
                }
            }
        }

        // Fallback to Windows native notification
        let body = format!("🏆 {}\n{}", achievement_name, description);
        self.show_notification(game_name, &body);
    }
}