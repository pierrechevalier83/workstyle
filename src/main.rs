mod config;
mod window_manager;

use lockfile::Lockfile;
use std::collections::BTreeMap;
use structopt::StructOpt;
use window_manager::{Window, WindowManager};

const LOCKFILE: &str = "/tmp/workstyle.lock";

#[derive(StructOpt)]
#[structopt(
    name = "workstyle",
    about = "\nWorkspaces with style!\n\nThis program will dynamically rename your workspaces to indicate which programs are running in each workspace. It uses the i3 ipc protocol, which makes it compatible with sway and i3.\n\nBy default, each program is mapped to a unicode character for concision.\n\nThe short description of each program is configurable. In the absence of a config file, one will be generated automatically.\nSee ${XDG_CONFIG_HOME}/workstyle/config.yml for  details."
)]
struct Options {}

fn make_new_workspace_names(
    workspaces: &BTreeMap<String, Vec<Window>>,
    icon_mappings: &[(String, String)],
    fallback_icon: &String,
) -> BTreeMap<String, String> {
    workspaces
        .iter()
        .map(|(name, windows)| {
            let new_name = pretty_windows(&windows, icon_mappings, fallback_icon);
            let num = name.split(":").next().unwrap();
            if new_name == "" {
                (name.clone(), num.to_string())
            } else {
                (name.clone(), format!("{}: {}", num, new_name))
            }
        })
        .collect()
}

fn pretty_window(
    window: &Window,
    icon_mappings: &[(String, String)],
    fallback_icon: &String,
) -> String {
    for (name, icon) in icon_mappings {
        if window.matches(name) {
            return icon.clone();
        }
    }
    log::error!("Couldn't identify window: {:?}", window);
    log::info!("Make sure to add an icon for this file in your config file!");
    fallback_icon.to_string()
}

fn pretty_windows(
    windows: &Vec<Window>,
    icon_mappings: &[(String, String)],
    fallback_icon: &String,
) -> String {
    let mut s = String::new();
    for window in windows {
        s.push_str(&pretty_window(window, icon_mappings, fallback_icon));
        s.push(' ');
    }
    s
}

fn main() {
    pretty_env_logger::init();

    let acquire_lock = Lockfile::create(LOCKFILE);
    if acquire_lock.is_err() {
        log::error!("Couldn't acquire lockfile: {:?}", LOCKFILE);
        log::error!(
            "If no other instance of workstyle is running, please delete the file manually."
        );
        return;
    }

    let _ = Options::from_args();
    let (mut wm, mut listener) = WindowManager::connect();
    let config_file = config::generate_config_file_if_absent();
    listener.window_events().for_each(|_| {
        let fallback_icon = config::get_fallback_icon(&config_file);
        let icon_mappings = config::get_icon_mappings(&config_file);
        let workspaces = wm.get_windows_in_each_workspace();
        let map = make_new_workspace_names(&workspaces, &icon_mappings, &fallback_icon);
        wm.rename_workspaces(map);

        std::thread::sleep(std::time::Duration::from_millis(100));
    });
}
