use std::{env, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub transparent: bool,
    #[serde(default)]
    pub glass: bool,
    #[serde(default)]
    pub corners: Corners,
    #[serde(default)]
    pub palette_index: usize,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Corners {
    #[default]
    Rounded,
    Square,
}

impl Settings {
    pub fn load() -> Self {
        fs::read(settings_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = settings_path();
        let Some(directory) = path.parent() else {
            return;
        };
        if fs::create_dir_all(directory).is_err() {
            return;
        }
        let Ok(bytes) = serde_json::to_vec_pretty(self) else {
            return;
        };
        let temporary = path.with_extension("json.tmp");
        if fs::write(&temporary, bytes).is_ok() {
            let _ = fs::remove_file(&path);
            let _ = fs::rename(temporary, path);
        }
    }
}

pub fn data_directory() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("ConfigCrate")
        .join("CodexTitlebarMeter")
}

fn settings_path() -> PathBuf {
    data_directory().join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_existing_settings_without_losing_accent() {
        let settings: Settings = serde_json::from_str(r#"{"palette_index":2}"#).unwrap();
        assert_eq!(settings.palette_index, 2);
        assert_eq!(settings.theme, Theme::Auto);
        assert!(!settings.transparent);
        assert!(!settings.glass);
        assert_eq!(settings.corners, Corners::Rounded);
    }

    #[test]
    fn appearance_round_trips() {
        let settings = Settings {
            palette_index: 5,
            theme: Theme::Light,
            transparent: true,
            glass: false,
            corners: Corners::Square,
        };
        let loaded: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(loaded.theme, Theme::Light);
        assert!(loaded.transparent);
        assert_eq!(loaded.palette_index, 5);
        assert_eq!(loaded.corners, Corners::Square);
    }

    #[test]
    fn default_palette_is_first() {
        assert_eq!(Settings::default().palette_index, 0);
    }

    #[test]
    fn experimental_glass_round_trips_without_losing_theme_or_accent() {
        let settings: Settings =
            serde_json::from_str(r#"{"theme":"dark","palette_index":5,"glass":true}"#).unwrap();
        let loaded: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(loaded.glass);
        assert!(!loaded.transparent);
        assert_eq!(loaded.theme, Theme::Dark);
        assert_eq!(loaded.palette_index, 5);
    }
}
