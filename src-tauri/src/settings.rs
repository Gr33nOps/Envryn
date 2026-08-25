//! Non-secret application preferences: auto-lock timeout, clipboard clear
//! delay.
//!
//! Deliberately not stored in the vault. These control *when* protective
//! behaviour kicks in, not what is protected, and the vault must be usable
//! before it is ever unlocked -- an auto-lock timeout that could only be read
//! from inside the very thing it locks would be a circular dependency.
//! Plain JSON, unencrypted, in the OS app-config directory.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;

const FILE_NAME: &str = "settings.json";

/// Range limits exist so a corrupted or hand-edited settings file cannot
/// disable auto-lock entirely by claiming an absurd timeout, or make the
/// clipboard clear so fast it clears before a paste completes.
const MIN_AUTO_LOCK_MINUTES: u32 = 1;
const MAX_AUTO_LOCK_MINUTES: u32 = 240;
const MIN_CLIPBOARD_SECONDS: u32 = 5;
const MAX_CLIPBOARD_SECONDS: u32 = 300;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    /// Whether the local AI subsystem may run at all. Defaults to `false` --
    /// AI is opt-in, never a silent default, matching specification section
    /// 2's "Local AI = OFF must leave every vault feature working." Turning
    /// this off does not just hide the UI; `ai.rs` refuses every AI command
    /// while it is false, so a stale cached frontend state can't route
    /// around the setting.
    pub ai_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 5,
            clipboard_clear_seconds: 30,
            ai_enabled: false,
        }
    }
}

impl AppSettings {
    fn clamp(mut self) -> Self {
        self.auto_lock_minutes = self
            .auto_lock_minutes
            .clamp(MIN_AUTO_LOCK_MINUTES, MAX_AUTO_LOCK_MINUTES);
        self.clipboard_clear_seconds = self
            .clipboard_clear_seconds
            .clamp(MIN_CLIPBOARD_SECONDS, MAX_CLIPBOARD_SECONDS);
        self
    }
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| "could not locate the application config directory".to_string())?;
    std::fs::create_dir_all(&dir)
        .map_err(|_| "could not create the config directory".to_string())?;
    Ok(dir.join(FILE_NAME))
}

pub fn load(app: &tauri::AppHandle) -> AppSettings {
    let Ok(path) = settings_path(app) else {
        return AppSettings::default();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return AppSettings::default();
    };
    // A corrupt settings file falls back to defaults rather than blocking
    // startup -- these values gate convenience behaviour, not vault access,
    // so failing safe here means "auto-lock uses the default," never "the
    // vault won't open."
    serde_json::from_slice::<AppSettings>(&bytes)
        .unwrap_or_default()
        .clamp()
}

fn save(app: &tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let bytes = serde_json::to_vec_pretty(&settings.clamp()).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings_get(app: tauri::AppHandle) -> AppSettings {
    load(&app)
}

#[tauri::command]
pub fn settings_set(app: tauri::AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let clamped = settings.clamp();
    save(&app, clamped)?;
    Ok(clamped)
}
