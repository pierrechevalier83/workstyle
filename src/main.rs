#[macro_use]
extern crate log;

mod config;
mod window_manager;

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Mutex;
use std::thread::{sleep, spawn};
use std::time::Duration;

use anyhow::{Context, Result};
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
/// 
/// If you prefer that empty workspaces be named with an icon,
/// instead of with a number, you can also specify:
/// 
/// [other]
/// use_empty_icon = true
/// empty_icon = ""

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

fn run() -> Result<()> {
    let mut wm = WindowManager::connect()?;
    info!("Successfully connected to WM");

    loop {
        // Wait for an OK(), which indicates an event that triggers a rename
        match wm.wait_for_event() {
            Err(..) => continue,
            _ => (),
        };
        
        // TODO: watch for changes using inotify and read the config only when needed
        let config = Config::new()?;

        let workspaces = wm.get_windows_in_each_workspace()?;
        for (name, windows) in workspaces {
            let new_name = pretty_windows(&config, &windows);
            let num = name
                .split(':')
                .next()
                .context("Unexpected workspace name")?;
            if new_name.is_empty() {
                let empty_name = if config.other.use_empty_icon {
                    config.empty_icon().into()
                } else {
                    num
                }.to_string();
                
                // Extra space matches other workspace icons.
                wm.rename_workspace(&name, &format!("{num}: {empty_name} "))?;
            } else {
                wm.rename_workspace(&name, &format!("{num}: {new_name}"))?;
            }
        }
    }
}

fn main() {
    env_logger::init();
    let _ = Args::parse();
    aquire_lock();
    loop {
        if let Err(e) = run() {
            error!("{e:#}");
            info!("Attempting to reconnect to the WM in 1 second");
            sleep(Duration::from_secs(1));
        }
    }
}
