use windows::Win32::Media::Audio::{PlaySoundA, SND_ALIAS, SND_ASYNC};
use windows::core::PCSTR;
use std::ffi::CString;
use std::thread;
use notify_rust::Notification;

pub struct NotificationManager;

impl NotificationManager {
    pub fn new() -> Self {
        Self
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

    pub fn show_game_detected(&self, game_name: &str) {
        self.show_notification("Game Save Monitor", &format!("{}\n▶ Game Detected - Monitoring saves...", game_name));
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
}