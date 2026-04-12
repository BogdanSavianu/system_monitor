use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GuiPersistentSettings {
    #[serde(default)]
    pub storage_enabled: bool,
    #[serde(default)]
    pub anomaly_enabled: bool,
}

impl Default for GuiPersistentSettings {
    fn default() -> Self {
        Self {
            storage_enabled: false,
            anomaly_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsFile {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    gui: GuiPersistentSettings,
}

fn default_version() -> u32 {
    1
}

impl Default for SettingsFile {
    fn default() -> Self {
        Self {
            version: default_version(),
            gui: GuiPersistentSettings::default(),
        }
    }
}

pub fn load_gui_settings() -> io::Result<GuiPersistentSettings> {
    let path = gui_settings_file_path();
    if !path.exists() {
        return Ok(GuiPersistentSettings::default());
    }

    let content = fs::read_to_string(path)?;
    let parsed: SettingsFile = toml::from_str(&content).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse settings.toml: {err}"),
        )
    })?;

    Ok(parsed.gui)
}

pub fn save_gui_settings(gui: GuiPersistentSettings) -> io::Result<()> {
    let path = gui_settings_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let settings = SettingsFile {
        version: default_version(),
        gui,
    };

    let encoded = toml::to_string_pretty(&settings)
        .map_err(|err| io::Error::other(format!("failed to serialize settings: {err}")))?;

    let tmp_path = path.with_extension("toml.tmp");
    let mut tmp_file = fs::File::create(&tmp_path)?;
    tmp_file.write_all(encoded.as_bytes())?;
    tmp_file.sync_all()?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

pub fn gui_settings_file_path() -> PathBuf {
    config_dir().join("system-monitor").join("settings.toml")
}

fn config_dir() -> PathBuf {
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg);
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".config");
    }

    PathBuf::from(".config")
}
