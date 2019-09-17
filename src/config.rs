use std::{
    env::current_dir,
    fs::{create_dir, File},
    io::{Read, Write},
    path::PathBuf,
};

const APP_NAME: &'static str = "sway_workspace_names";

fn config_file() -> PathBuf {
    let mut path_to_config = dirs::config_dir().unwrap_or(current_dir().unwrap());
    path_to_config.push(APP_NAME);
    if !path_to_config.exists() {
        create_dir(path_to_config.clone()).unwrap();
    }
    path_to_config.push("config.yml");
    path_to_config
}

pub(super) fn generate_config_file_if_absent() {
    let config_file = config_file();
    if !config_file.exists() {
        println!("creating {:#?}", config_file);
        let mut config_file = File::create(config_file).unwrap();
        let content = include_bytes!("default_config.yml");
        config_file.write_all(content).unwrap();
    }
}

pub(super) fn get_icon_mappings() -> Vec<(String, String)> {
    let mut config_file = File::open(config_file()).unwrap();
    let mut content = String::new();
    config_file.read_to_string(&mut content).unwrap();
    serde_yaml::from_str(&content).unwrap()
}
