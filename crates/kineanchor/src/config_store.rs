// SPDX-License-Identifier: GPL-3.0-or-later

use directories::ProjectDirs;
use kineanchor_core::Config;
use std::fs;
use std::path::PathBuf;

fn config_path() -> Option<PathBuf> {
    let proj = ProjectDirs::from("io.github.nandazahran", "", "kineanchor")?;
    let dir = proj.config_dir();
    let _ = fs::create_dir_all(dir);
    Some(dir.join("config.json"))
}

pub fn load_config() -> Config {
    if let Some(path) = config_path() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Config>(&data) {
                return cfg;
            }
        }
    }
    Config::default()
}

pub fn save_config(config: &Config) {
    if let Some(path) = config_path() {
        if let Ok(data) = serde_json::to_string_pretty(config) {
            let _ = fs::write(path, data);
        }
    }
}
