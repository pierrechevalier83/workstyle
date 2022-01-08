mod config;
mod window_manager;

use crate::window_manager::EventStream;
use futures::stream::StreamExt;
use lockfile::Lockfile;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook_tokio::Signals;
use std::{collections::BTreeMap, process::exit};
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
    fallback_icon: &str,
) -> Result<BTreeMap<String, String>, &'static str> {
    workspaces
        .iter()
        .map(|(name, windows)| {
            let new_name = pretty_windows(windows, icon_mappings, fallback_icon);
            let num = name.split(':').next().ok_or("Unexpected workspace name")?;
            if new_name.is_empty() {
                Ok((name.clone(), num.to_string()))
            } else {
                Ok((name.clone(), format!("{}: {}", num, new_name)))
            }
        })
        .collect()
}

fn pretty_window(
    window: &Window,
    icon_mappings: &[(String, String)],
    fallback_icon: &str,
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
    windows: &[Window],
    icon_mappings: &[(String, String)],
    fallback_icon: &str,
) -> String {
    let mut s = String::new();
    for window in windows {
        s.push_str(&pretty_window(window, icon_mappings, fallback_icon));
        s.push(' ');
    }
    s
}

async fn process_events(
    wm: &mut WindowManager,
    stream: &mut EventStream,
) -> Result<(), &'static str> {
    let config_file = config::generate_config_file_if_absent();
    while let Ok(_event) = stream.next().await {
        let fallback_icon = config::get_fallback_icon(&config_file);
        let icon_mappings = config::get_icon_mappings(&config_file);

        let workspaces = wm.get_windows_in_each_workspace().await.map_err(|e| {
            log::error!("{}", e);
            e
        })?;
        let map =
            make_new_workspace_names(&workspaces, &icon_mappings, &fallback_icon).map_err(|e| {
                log::error!("{}", e);
                e
            })?;
        wm.rename_workspaces(map).await.map_err(|e| {
            log::error!("{}", e);
            e
        })?;
    }
    Err("Can't get next event")
}

async fn handle_signals(mut signals: Signals, lock: Lockfile) {
    while let Some(signal) = signals.next().await {
        if TERM_SIGNALS.contains(&signal) {
            log::info!("Received termination signal: {}. Exiting...", signal);
            drop(lock);
            exit(signal);
        }
    }
}

async fn main_loop(mut wm: WindowManager, mut stream: EventStream) {
    loop {
        if process_events(&mut wm, &mut stream).await.is_err() {
            log::info!("Couldn't process WM events. The WM might have been terminated");
            log::info!("Attempting to reconnect to the WM");
            if let Ok((w, s)) = WindowManager::connect().await {
                wm = w;
                stream = s;
                log::info!("Successfully reconnected to WM");
            } else {
                log::info!("Failed to reconnect to WM. Will try again in 1 second");
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    pretty_env_logger::init();
    let _ = Options::from_args();

    let lock = Lockfile::create(LOCKFILE)
        .map_err(|_| "Couldn't acquire lock: /tmp/workstyle.lock already exists")?;

    let signals = Signals::new(TERM_SIGNALS).expect("Failed to create Signals");
    let handle = signals.handle();
    let termination_signals_task = tokio::spawn(handle_signals(signals, lock));

    let (wm, stream) = WindowManager::connect().await?;
    tokio::spawn(main_loop(wm, stream))
        .await
        .map_err(|_| "Error in main loop")?;

    handle.close();
    termination_signals_task
        .await
        .map_err(|_| "Terminated by signal")?;
    Ok(())
}
