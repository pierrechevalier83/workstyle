#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

mod config;
mod window_manager;

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Mutex;
use std::thread::{sleep, spawn};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use config::Config;
use lockfile::Lockfile;
use once_cell::sync::Lazy;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::iterator::Signals;
use window_manager::{Window, WindowManager};

/// Workspaces with style!
///
/// This program will dynamically rename your workspaces to indicate which
/// programs are running in each workspace. It uses the i3 ipc protocol, which
/// makes it compatible with sway and i3.
///
/// By default, each program is mapped to a unicode character for concision.
///
/// The short description of each program is configurable. In the absence of a
/// config file, one will be generated automatically.
/// See ${XDG_CONFIG_HOME}/workstyle/config.yml for  details.
///
/// If you prefer not to have multiple copies of the same icon when there are
/// multiple matching windows, set this config option:
///
/// [other]
/// deduplicate_icons = true
#[derive(Parser, Debug)]
#[clap(version, about)]
struct Args;

static LOCK: Lazy<Mutex<Option<Lockfile>>> =
    Lazy::new(|| Mutex::new(Lockfile::create(lockfile_path()).ok()));

fn pretty_window(config: &Config, window: &Window) -> String {
    for (name, icon) in &config.mappings {
        if window.matches(name) {
            return icon.clone();
        }
    }
    error!("Couldn't identify window: {window:?}");
    info!("Make sure to add an icon for this file in your config file!");
    config.fallback_icon().into()
}

fn pretty_windows(config: &Config, windows: &[Window]) -> String {
    let mut s = String::new();
    if config.other.deduplicate_icons {
        let mut set = HashSet::new();
        for window in windows {
            let icon = pretty_window(config, window);
            if set.get(&icon).is_none() {
                s.push_str(&icon);
                s.push(' ');
                set.insert(icon);
            }
        }
    } else {
        for window in windows {
            s.push_str(&pretty_window(config, window));
            s.push(' ');
        }
    }
    s
}

fn process_events(mut wm: WindowManager) -> Result<()> {
    loop {
        // TODO: watch for changes using inotify and read the config only when needed
        let config = Config::new()?;

        let workspaces = wm.get_windows_in_each_workspace()?;
        for (name, windows) in workspaces {
            let new_name = pretty_windows(&config, &windows);
            let num = name
                .split(':')
                .next()
                .ok_or_else(|| anyhow!("Unexpected workspace name"))?;
            if new_name.is_empty() {
                wm.rename_workspace(&name, num)?;
            } else {
                wm.rename_workspace(&name, &format!("{num}: {new_name}"))?;
            }
        }

        wm.wait_for_event()?;
    }
}

fn lockfile_path() -> PathBuf {
    let mut lockfile_path = match dirs::runtime_dir() {
        Some(path) => path,
        None => PathBuf::from("/tmp"),
    };
    lockfile_path.push("workstyle.lock");
    lockfile_path
}

fn aquire_lock() {
    // Try to aquire the lock
    if LOCK.lock().unwrap().is_none() {
        error!("Failed to aquire the lock");
        exit(1);
    }

    // Drop the lock on exit
    let mut signals = Signals::new(TERM_SIGNALS).expect("Failed to create signals iterator");
    spawn(move || {
        let _ = signals.forever().next();
        drop(LOCK.lock().unwrap().take());
        exit(0);
    });

    // Drop the lock on panic
    std::panic::set_hook(Box::new(|info| {
        error!("{info}");
        if let Ok(mut lock) = LOCK.lock() {
            drop(lock.take());
        }
    }));
}

fn main() -> Result<()> {
    let _ = Args::parse();
    aquire_lock();

    env_logger::init();
    loop {
        let wm = loop {
            match WindowManager::connect() {
                Ok(wm) => {
                    info!("Successfully connected to WM");
                    break wm;
                }
                Err(error) => {
                    error!("{error}");
                    error!("Failed to connect to WM. Will try again in 1 second");
                    sleep(Duration::from_secs(1));
                }
            }
        };

        if let Err(error) = process_events(wm) {
            error!("Error: {error}");
            error!("Couldn't process WM events. The WM might have been terminated");
            info!("Attempting to reconnect to the WM");
        }
    }
}
