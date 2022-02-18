#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Mutex;
use std::thread::{sleep, spawn};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use lockfile::Lockfile;
use once_cell::sync::Lazy;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::iterator::Signals;
use swayipc::EventStream;

mod config;
mod window_manager;

use config::Config;
use window_manager::{Window, WindowManager};

static LOCK: Lazy<Mutex<Option<Lockfile>>> =
    Lazy::new(|| Mutex::new(Lockfile::create(lockfile_path()).ok()));

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
#[derive(Parser, Debug)]
#[clap(version, about)]
struct Args;

fn pretty_window(config: &Config, window: &Window) -> String {
    for (name, icon) in &config.mappings {
        if window.matches(name) {
            return icon.clone();
        }
    }
    warn!("Couldn't identify window: {:?} Make sure to add an icon for this file in your config file!", window);
    config.fallback_icon().into()
}

fn pretty_windows(config: &Config, windows: &[Window]) -> String {
    let mut s = String::new();
    if config.other.merge {
        let mut set = BTreeSet::new();
        for window in windows {
            set.insert(pretty_window(config, window));
        }
        for v in set {
            s.push_str(&v);
            s.push(' ');
        }
    } else {
        for window in windows {
            s.push_str(&pretty_window(config, window));
            s.push(' ');
        }
    }
    s
}

fn process_events(config: &Config, mut wm: WindowManager, stream: EventStream) -> Result<()> {
    for _event in stream {
        let workspaces = wm.get_windows_in_each_workspace()?;
        for (name, windows) in workspaces {
            let new_name = pretty_windows(config, &windows);
            let num = name
                .split(':')
                .next()
                .ok_or(anyhow!("Unexpected workspace name"))?;
            if new_name.is_empty() {
                wm.rename_workspace(&name, num)?;
            } else {
                wm.rename_workspace(&name, &format!("{num}: {new_name}"))?;
            }
        }
    }
    bail!("Can't get next event")
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
        panic!("Failed to aquire the lock");
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
    let config = Config::new()?;
    loop {
        let wm;
        let stream;

        loop {
            if let Ok((w, s)) = WindowManager::connect() {
                wm = w;
                stream = s;
                info!("Successfully connected to WM");
                break;
            } else {
                error!("Failed to connect to WM. Will try again in 1 second");
                sleep(Duration::from_secs(1));
            }
        }

        if let Err(error) = process_events(&config, wm, stream) {
            error!("Error: {error}");
            error!("Couldn't process WM events. The WM might have been terminated");
            info!("Attempting to reconnect to the WM");
        }
    }
}
