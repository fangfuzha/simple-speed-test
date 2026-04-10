use serde::{Deserialize, Serialize};
use std::{env, fs, io, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopSettings {
    pub autostart: bool,
    pub open_browser_on_start: bool,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            autostart: false,
            open_browser_on_start: true,
        }
    }
}

impl DesktopSettings {
    pub fn load() -> io::Result<Self> {
        let path = settings_file_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    pub fn save(&self) -> io::Result<()> {
        let path = settings_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self).unwrap())
    }
}

pub fn settings_file_path() -> PathBuf {
    app_config_dir().join("settings.json")
}

pub fn app_config_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        if let Ok(appdata) = env::var("APPDATA") {
            return PathBuf::from(appdata).join("speed-test");
        }
    }

    if cfg!(target_os = "macos") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("speed-test");
        }
    }

    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("speed-test");
    }

    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join(".config").join("speed-test");
    }

    PathBuf::from("speed-test")
}

pub fn startup_entry_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        if let Ok(appdata) = env::var("APPDATA") {
            return PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("Startup")
                .join("speed-test-autostart.bat");
        }
    }

    if cfg!(target_os = "macos") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("LaunchAgents")
                .join("com.speed-test.autostart.plist");
        }
    }

    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home)
            .join("autostart")
            .join("speed-test.desktop");
    }

    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("autostart")
            .join("speed-test.desktop");
    }

    PathBuf::from("speed-test-autostart")
}
