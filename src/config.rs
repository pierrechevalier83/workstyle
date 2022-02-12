use serde_derive::Deserialize;
use std::path::Path;
use std::{
    fs::{create_dir, File},
    io::{BufReader, Error, ErrorKind, Read, Write},
    path::PathBuf,
};
use toml::Value;

const APP_NAME: &str = "workstyle";
const DEFAULT_FALLBACK_ICON: &str = " ";

fn config_file() -> Result<PathBuf, Error> {
    let mut user_path = dirs::config_dir()
        .ok_or_else(|| Error::new(ErrorKind::Other, "Missing default config dir"))?;
    user_path.push(APP_NAME);
    if !user_path.exists() {
        create_dir(user_path.clone())?;
    }
    user_path.push("config.toml");

    // If user-level config doesn't exist, try system-level config.
    if !user_path.exists() {
        let sys_path = PathBuf::from(format!("/etc/xdg/{}/config.toml", APP_NAME));
        if sys_path.exists() {
            return Ok(sys_path)
        }
    }
    Ok(user_path)
}

pub(super) fn generate_config_file_if_absent() -> Result<PathBuf, Error> {
    let config_file = config_file()?;
    if !config_file.exists() {
        let mut config_file = File::create(&config_file)?;
        let content = include_bytes!("default_config.toml");
        config_file.write_all(content)?;
    }
    Ok(config_file)
}

// I had rather simply deserialize the map with serde, but I don't know what to deserialize into to
// preserve ordering.
fn try_from_toml_value(value: &Value) -> Result<Vec<(String, String)>, String> {
    match value {
        Value::Table(map) => Ok(map
            .into_iter()
            .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
            .collect()),
        _ => Err("Expected a map".to_string()),
    }
}

fn get_icon_mappings_from_config(config: &Path) -> Result<Vec<(String, String)>, Error> {
    let mut config_file = File::open(config)?;
    let mut content = String::new();
    config_file.read_to_string(&mut content)?;
    try_from_toml_value(&content.parse::<toml::Value>().map_err(|e| {
        log::error!(
            "Error parsing configuration file.\nInvalid syntax in {:#?}.\n{}",
            config,
            e
        );
        Error::new(ErrorKind::Other, "Invalid configuration file")
    })?)
    .map_err(|e| {
        log::error!("{}", e);
        Error::new(ErrorKind::Other, "Invalid configuration file")
    })
}

fn get_icon_mappings_from_default_config() -> Vec<(String, String)> {
    try_from_toml_value(&String::from_utf8(include_bytes!("default_config.toml").to_vec()).expect("Expected utf-8 encoded string in default_config.toml").parse::<Value>()
        .expect("The default config isn't user generated, so we assumed it was correct. This will teach us not to trust programmers.")).expect("Bang!")
}

pub(super) fn get_icon_mappings(config: &Result<PathBuf, Error>) -> Vec<(String, String)> {
    if let Ok(config) = config {
        if let Ok(content) = get_icon_mappings_from_config(config) {
            return content;
        }
    }
    get_icon_mappings_from_default_config()
}

#[derive(Debug, Deserialize)]
struct Config {
    other: Option<ExtraConfig>,
}

#[derive(Debug, Deserialize)]
struct ExtraConfig {
    #[serde(default = "ExtraConfig::default_fallback_icon")]
    fallback_icon: String,
}

impl ExtraConfig {
    fn default_fallback_icon() -> String {
        DEFAULT_FALLBACK_ICON.to_string()
    }
}

pub(super) fn get_fallback_icon(config: &Result<PathBuf, Error>) -> String {
    let config_path = config.as_ref().unwrap().to_str().unwrap();
    let mut config_file = BufReader::new(
        File::open(config_path).unwrap_or_else(|_| panic!("Failed to open file: {}", config_path)),
    );
    let mut content = String::new();
    config_file
        .read_to_string(&mut content)
        .expect("Failed to read file");
    let config: Config = toml::from_str(&content).unwrap();
    if let Some(other) = config.other {
        other.fallback_icon
    } else {
        DEFAULT_FALLBACK_ICON.to_string()
    }
}
