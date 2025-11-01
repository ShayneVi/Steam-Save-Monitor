use tauri::{Manager, Window};
use windows::Win32::Foundation::{RECT, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, GetWindowLongW,
    SetWindowLongW, GWL_STYLE, GWL_EXSTYLE, WS_POPUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WINDOW_EX_STYLE, HWND_TOPMOST, SetWindowPos, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE,
};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// Represents the display mode of a window
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowMode {
    Fullscreen,
    BorderlessWindowed,
    Windowed,
}

/// Manages the overlay notification window
pub struct OverlayManager {
    overlay_window: Option<Window>,
}

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            overlay_window: None,
        }
    }

    /// Initialize the overlay window
    pub fn init(&mut self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        // Get or create overlay window
        match app_handle.get_window("overlay") {
            Some(window) => {
                self.overlay_window = Some(window);
                Ok(())
            }
            None => Err("Overlay window not found".to_string()),
        }
    }

    /// Detect the window mode of a given window by its title
    pub fn detect_window_mode(window_title: &str) -> WindowMode {
        unsafe {
            // Convert window title to wide string
            let wide_title: Vec<u16> = OsStr::new(window_title)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // Find the window by title
            let hwnd = FindWindowW(None, windows::core::PCWSTR(wide_title.as_ptr()));

            if hwnd.0 == 0 {
                // Window not found, default to windowed
                return WindowMode::Windowed;
            }

            // Get window style
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);

            // Get window rect
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);

            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            // Check if window is fullscreen
            // A fullscreen window typically:
            // 1. Has no border/title bar (WS_POPUP style)
            // 2. Covers the entire screen
            let has_popup_style = (style as u32) & WS_POPUP.0 != 0;

            // Get screen dimensions (simplified - assumes primary monitor)
            let screen_width = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN
            );
            let screen_height = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN
            );

            let covers_screen = width >= screen_width && height >= screen_height;

            if has_popup_style && covers_screen {
                // This is likely exclusive fullscreen
                WindowMode::Fullscreen
            } else if has_popup_style {
                // Popup style without covering screen = borderless windowed
                WindowMode::BorderlessWindowed
            } else {
                // Regular windowed mode
                WindowMode::Windowed
            }
        }
    }

    /// Set window extended style to prevent activation/focus stealing
    fn set_no_activate(hwnd: HWND) -> Result<(), String> {
        unsafe {
            // Get current extended style
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);

            // Add WS_EX_NOACTIVATE and WS_EX_TOOLWINDOW flags
            let new_ex_style = ex_style | (WS_EX_NOACTIVATE.0 as i32) | (WS_EX_TOOLWINDOW.0 as i32);

            // Set new extended style
            SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex_style);

            // Update window position with SWP_NOACTIVATE to ensure no focus stealing
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );

            Ok(())
        }
    }

    /// Show the overlay window with notification data
    pub fn show_overlay(&self, notification_type: &str, data: serde_json::Value) -> Result<(), String> {
        if let Some(window) = &self.overlay_window {
            // Get HWND and set no-activate style
            if let Ok(hwnd) = window.hwnd() {
                let hwnd = HWND(hwnd.0 as isize);
                Self::set_no_activate(hwnd)?;
            }

            // Show the overlay window without activating it
            window.show().map_err(|e| format!("Failed to show overlay: {}", e))?;

            // Emit event to overlay window with notification data
            window
                .emit("show-notification", (notification_type, data))
                .map_err(|e| format!("Failed to emit notification event: {}", e))?;

            Ok(())
        } else {
            Err("Overlay window not initialized".to_string())
        }
    }

    /// Hide the overlay window
    pub fn hide_overlay(&self) -> Result<(), String> {
        if let Some(window) = &self.overlay_window {
            window.hide().map_err(|e| format!("Failed to hide overlay: {}", e))?;
            Ok(())
        } else {
            Err("Overlay window not initialized".to_string())
        }
    }

    /// Check if we should use overlay or fallback to native notifications
    pub fn should_use_overlay(game_title: &str) -> bool {
        let mode = Self::detect_window_mode(game_title);

        // Use overlay for borderless and windowed, fallback to native for fullscreen
        match mode {
            WindowMode::Fullscreen => false,
            WindowMode::BorderlessWindowed | WindowMode::Windowed => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_mode_detection() {
        // This is a placeholder test - actual testing requires a window to be present
        let mode = OverlayManager::detect_window_mode("NonExistentWindow");
        assert_eq!(mode, WindowMode::Windowed);
    }
}
