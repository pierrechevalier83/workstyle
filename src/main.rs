#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

mod config;
mod window_manager;

use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use futures::stream::StreamExt;
use lockfile::Lockfile;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook_tokio::Signals;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::exit;
use swayipc_async::EventStream;
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
#[derive(Parser, Debug)]
#[clap(version, about)]
struct Args;

fn pretty_window(config: &Config, window: &Window) -> String {
    for (name, icon) in &config.mappings {
        if window.matches(name) {
            return icon.clone();
        }
    }
    warn!("Couldn't identify window: {window:?}");
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

async fn process_events(wm: &mut WindowManager, stream: &mut EventStream) -> Result<()> {
    while let Some(_event) = stream.next().await {
        // TODO: watch config file with inotify and read it only when necessary
        let config = Config::new()?;
        let workspaces = wm
            .get_windows_in_each_workspace()
            .await
            .map_err(|e| anyhow!(e))?;
        for (name, windows) in workspaces {
            let new_name = pretty_windows(&config, &windows);
            let num = name
                .split(':')
                .next()
                .ok_or_else(|| anyhow!("Unexpected workspace name"))?;
            if new_name.is_empty() {
                wm.rename_workspace(&name, num).await?;
            } else {
                wm.rename_workspace(&name, &format!("{num}: {new_name}"))
                    .await?;
            }
        }
    }
    bail!("Can't get next event")
}

async fn handle_signals(mut signals: Signals, lock: Lockfile) {
    while let Some(signal) = signals.next().await {
        if TERM_SIGNALS.contains(&signal) {
            info!("Received termination signal: {}. Exiting...", signal);
            drop(lock);
            exit(signal);
        }
    }
}

async fn main_loop(mut wm: WindowManager, mut stream: EventStream) {
    loop {
        if let Err(error) = process_events(&mut wm, &mut stream).await {
            error!("{error}");
            info!("Couldn't process WM events. The WM might have been terminated");
            info!("Attempting to reconnect to the WM");
            if let Ok((w, s)) = WindowManager::connect().await {
                wm = w;
                stream = s;
                info!("Successfully reconnected to WM");
            } else {
                info!("Failed to reconnect to WM. Will try again in 1 second");
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    let _ = Args::parse();

    let mut lockfile_path = match dirs::runtime_dir() {
        Some(path) => path,
        None => PathBuf::from("/tmp"),
    };
    lockfile_path.push("workstyle.lock");

    let path_str = String::from(lockfile_path.to_str().unwrap());

    let lock = match Lockfile::create(lockfile_path) {
        Ok(lock) => lock,
        Err(err) => panic!("Unrecoverable error: {}, {}", err.into_inner(), path_str),
    };

    let signals = Signals::new(TERM_SIGNALS).expect("Failed to create Signals");
    let handle = signals.handle();
    let termination_signals_task = tokio::spawn(handle_signals(signals, lock));

    let (wm, stream) = WindowManager::connect().await?;
    tokio::spawn(main_loop(wm, stream))
        .await
        .context("Error in main loop")?;

    handle.close();
    termination_signals_task
        .await
        .context("Terminated by signal")?;
    Ok(())
}
