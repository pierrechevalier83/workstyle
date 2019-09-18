use std::{
    fs::{create_dir, File},
    io::{Error, ErrorKind, Read, Write},
    path::PathBuf,
};

const APP_NAME: &'static str = "workstyle";

fn config_file() -> Result<PathBuf, Error> {
    let mut path_to_config =
        dirs::config_dir().ok_or(Error::new(ErrorKind::Other, "Missing default config dir"))?;
    path_to_config.push(APP_NAME);
    if !path_to_config.exists() {
        create_dir(path_to_config.clone())?;
    }
    path_to_config.push("config.yml");
    Ok(path_to_config)
}

pub(super) fn generate_config_file_if_absent() -> Result<PathBuf, Error> {
    let config_file = config_file()?;
    if !config_file.exists() {
        let mut config_file = File::create(&config_file)?;
        let content = include_bytes!("default_config.yml");
        config_file.write_all(content)?;
    }
    Ok(config_file)
}

fn get_icon_mappings_from_config(config: &PathBuf) -> Result<Vec<(String, String)>, Error> {
    let mut config_file = File::open(config)?;
    let mut content = String::new();
    config_file.read_to_string(&mut content)?;
    // TODO: log that error
    serde_yaml::from_str(&content)
        .map_err(|_| Error::new(ErrorKind::Other, "Invalid configuration file"))
}

fn get_icon_mappings_from_default_config() -> Vec<(String, String)> {
    serde_yaml::from_slice(include_bytes!("default_config.yml"))
        .expect("The default config isn't user generated, so we assumed it was correct. This will teach us not to trust programmers.")
}

pub(super) fn get_icon_mappings(config: &Result<PathBuf, Error>) -> Vec<(String, String)> {
    if let Ok(config) = config {
        if let Ok(content) = get_icon_mappings_from_config(&config) {
            return content;
        }
    }
    get_icon_mappings_from_default_config()
}
