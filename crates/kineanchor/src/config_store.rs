// SPDX-License-Identifier: GPL-3.0-or-later

use crate::net_provider::PersistedPairing;
use directories::ProjectDirs;
use kineanchor_core::Config;
use std::fs;
use std::path::PathBuf;

fn config_dir() -> Option<PathBuf> {
    let proj = ProjectDirs::from("io.github.nandazahran", "", "kineanchor")?;
    let dir = proj.config_dir();
    let _ = fs::create_dir_all(dir);
    Some(dir.to_path_buf())
}

fn config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("config.json"))
}

/// Path of the persisted settings file, for the About screen's "stored on this computer" box.
pub fn config_file_path() -> Option<PathBuf> {
    config_path()
}

fn pairing_path() -> Option<PathBuf> {
    Some(config_dir()?.join("pairing.json"))
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

pub fn load_pairing() -> PersistedPairing {
    if let Some(path) = pairing_path() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(p) = serde_json::from_str::<PersistedPairing>(&data) {
                return p;
            }
        }
    }
    PersistedPairing::default()
}

pub fn save_pairing(pairing: &PersistedPairing) {
    if let Some(path) = pairing_path() {
        if let Ok(data) = serde_json::to_string_pretty(pairing) {
            let _ = fs::write(path, data);
        }
    }
}
