use crate::EnforceWindowManager;
use anyhow::{anyhow, bail, Context, Result};
use hyprland::data::{Clients, Version, Workspaces};
use hyprland::dispatch::{Dispatch, DispatchType};
use hyprland::event_listener::EventListener;
use hyprland::shared::HyprData;
use itertools::Itertools;
use std::collections::BTreeMap;
use std::sync::{mpsc, mpsc::Receiver};
use std::thread;
use swayipc::{Connection, EventStream, EventType, Node, NodeType};

trait NodeExt {
    fn is_workspace(&self) -> bool;
    fn is_window(&self) -> bool;
    fn name(&self) -> Option<String>;
    fn app_id(&self) -> Option<String>;
    fn window_properties_class(&self) -> Option<String>;
    fn windows_in_node(&self) -> Vec<Window>;
    fn workspaces_in_node(&self) -> Result<BTreeMap<String, Vec<Window>>>;
}

impl NodeExt for Node {
    fn is_workspace(&self) -> bool {
        // `__i3_scratch` is a special workspace that connot be renamed, so we just skip it
        self.name.as_deref() != Some("__i3_scratch") && self.node_type == NodeType::Workspace
    }
    fn is_window(&self) -> bool {
        matches!(self.node_type, NodeType::Con | NodeType::FloatingCon)
    }
    fn name(&self) -> Option<String> {
        self.name.clone()
    }
    fn app_id(&self) -> Option<String> {
        self.app_id.clone()
    }
    fn window_properties_class(&self) -> Option<String> {
        self.window_properties
            .as_ref()
            .and_then(|prop| prop.class.clone())
    }
    /// Recursively find all windows names in this node
    fn windows_in_node(&self) -> Vec<Window> {
        let mut res = Vec::new();
        for node in self.nodes.iter().chain(self.floating_nodes.iter()) {
            res.extend(node.windows_in_node());
            if node.is_window() {
                if let Some(window) = Window::from_node(node) {
                    res.push(window);
                }
            }
        }
        res
    }
    /// Recursively find all workspaces in this node and the list of open windows for each of these
    /// workspaces
    fn workspaces_in_node(&self) -> Result<BTreeMap<String, Vec<Window>>> {
        let mut res = BTreeMap::new();
        for node in &self.nodes {
            if node.is_workspace() {
                res.insert(
                    node.name().context("Expected some node name")?,
                    node.windows_in_node(),
                );
            } else {
                let workspaces = node.workspaces_in_node()?;
                for (k, v) in workspaces {
                    res.insert(k, v);
                }
            }
        }
        Ok(res)
    }
}

#[derive(Debug)]
pub struct Window {
    pub(crate) name: Option<String>,
    pub(crate) app_id: Option<String>,
    pub(crate) window_properties_class: Option<String>,
}

impl Window {
    fn from_node(node: &Node) -> Option<Self> {
        if node.is_window() {
            let name = node.name();
            let app_id = node.app_id();
            let window_properties_class = node.window_properties_class();
            if name.is_some() || app_id.is_some() || window_properties_class.is_some() {
                Some(Self {
                    name,
                    app_id,
                    window_properties_class,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
    fn exists(&self) -> bool {
        self.name.is_some() || self.app_id.is_some() || self.window_properties_class.is_some()
    }
    pub fn matches(&self, pattern: &str) -> bool {
        self.name
            .as_ref()
            .map(|s| s.to_lowercase().contains(pattern))
            .unwrap_or(false)
            || self
                .app_id
                .as_ref()
                .map(|s| s.to_lowercase().contains(pattern))
                .unwrap_or(false)
            || self
                .window_properties_class
                .as_ref()
                .map(|s| s.to_lowercase().contains(pattern))
                .unwrap_or(false)
    }
}

pub trait WM {
    fn connect(enforce: Option<EnforceWindowManager>) -> Result<Box<Self>>;
    fn get_windows_in_each_workspace(&mut self) -> Result<BTreeMap<String, Vec<Window>>>;
    fn rename_workspace(&mut self, old: &str, new: &str) -> Result<()>;
    fn wait_for_event(&mut self) -> Result<()>;
}

pub enum WindowManager {
    SwayOrI3(Box<SwayOrI3>),
    Hyprland(Box<Hyprland>),
}

impl WM for WindowManager {
    fn connect(enforce: Option<EnforceWindowManager>) -> Result<Box<Self>> {
        let connect_to_sway_or_i3 =
            || SwayOrI3::connect(enforce).map(|wm| Box::new(Self::SwayOrI3(wm)));
        let connect_to_hyprland =
            || Hyprland::connect(enforce).map(|wm| Box::new(Self::Hyprland(wm)));
        match enforce {
            Some(EnforceWindowManager::SwayOrI3) => connect_to_sway_or_i3(),
            Some(EnforceWindowManager::Hyprland) => connect_to_hyprland(),
            None => {
                connect_to_sway_or_i3().or_else(|_| connect_to_hyprland()).map_err(|_| anyhow!("Couldn't connect to the window manager. Only Sway, I3 and Hyprland are officially supported."))
            }

        }
    }
    fn get_windows_in_each_workspace(&mut self) -> Result<BTreeMap<String, Vec<Window>>> {
        match self {
            Self::SwayOrI3(wm) => wm.get_windows_in_each_workspace(),
            Self::Hyprland(wm) => wm.get_windows_in_each_workspace(),
        }
    }
    fn rename_workspace(&mut self, old: &str, new: &str) -> Result<()> {
        match self {
            Self::SwayOrI3(wm) => wm.rename_workspace(old, new),
            Self::Hyprland(wm) => wm.rename_workspace(old, new),
        }
    }
    fn wait_for_event(&mut self) -> Result<()> {
        match self {
            Self::SwayOrI3(wm) => wm.wait_for_event(),
            Self::Hyprland(wm) => wm.wait_for_event(),
        }
    }
}

pub struct Hyprland {
    rx: Receiver<()>,
}

impl WM for Hyprland {
    fn connect(enforce: Option<EnforceWindowManager>) -> Result<Box<Self>> {
        match enforce {
            None | Some(EnforceWindowManager::Hyprland) => {
                Version::get()?;
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let mut listener = EventListener::new();
                    let tx_clone = tx.clone();
                    listener.add_window_open_handler(move |_| {
                        tx_clone.send(()).unwrap();
                    });
                    let tx_clone = tx.clone();
                    listener.add_window_close_handler(move |_| {
                        tx_clone.send(()).unwrap();
                    });
                    let tx_clone = tx.clone();
                    listener.add_window_moved_handler(move |_| {
                        tx_clone.send(()).unwrap();
                    });
                    let tx_clone = tx.clone();
                    listener.add_layer_open_handler(move |_| {
                        tx_clone.send(()).unwrap();
                    });
                    listener.add_layer_closed_handler(move |_| {
                        tx.send(()).unwrap();
                    });
                    listener.start_listener().map_err(|e| anyhow!(e)).unwrap();
                });
                Ok(Box::new(Self { rx }))
            }
            _ => {
                bail!("Not connecting to Hyprland as we've been explicitly asked not to")
            }
        }
    }

    fn get_windows_in_each_workspace(&mut self) -> Result<BTreeMap<String, Vec<Window>>> {
        let empty_workspaces = Workspaces::get()
            .context("Failed to get workspaces")?
            .filter_map(|workspace| {
                if workspace.windows == 0 {
                    Some((format!("{}", workspace.id), Vec::new()))
                } else {
                    None
                }
            });
        Ok(Clients::get()
            .context("Failed to get clients")?
            .map(|client| {
                (
                    client.workspace.id,
                    (
                        // Keep the position so the order of the icons matches the order of the
                        // windows on the screen, from left to right then top to bottom
                        (
                            client.at.1, /*y position in pixel*/
                            client.at.0, /* x position in px */
                        ),
                        Window {
                            name: match client.title.as_str() {
                                "" => None,
                                s => Some(s.to_string()),
                            },
                            app_id: None,
                            window_properties_class: match client.class.as_str() {
                                "" => None,
                                s => Some(s.to_string()),
                            },
                        },
                    ),
                )
            })
            .into_group_map()
            .into_iter()
            .map(|(k, mut v)| {
                // Sort by position
                v.sort_by(|(l, _), (r, _)| l.cmp(r));
                (
                    format!("{k}"),
                    v.into_iter()
                        // We don't need the position anymore. Dismiss it
                        .map(|(_pos, w)| w)
                        .filter(|w| w.exists())
                        .collect(),
                )
            })
            .chain(empty_workspaces)
            .collect())
    }

    fn rename_workspace(&mut self, old: &str, new: &str) -> Result<()> {
        Dispatch::call(DispatchType::RenameWorkspace(
            old.parse().context("Failed to parse workspace id")?,
            Some(new),
        ))
        .context(format!("Failed to rename workspace from {old} to {new}"))
    }

    fn wait_for_event(&mut self) -> Result<()> {
        self.rx.recv().context("Failed to wait for event")
    }
}

pub struct SwayOrI3 {
    connection: Connection,
    events: EventStream,
}

impl WM for SwayOrI3 {
    fn connect(enforce: Option<EnforceWindowManager>) -> Result<Box<Self>> {
        match enforce {
            None | Some(EnforceWindowManager::SwayOrI3) => Ok(Box::new(Self {
                connection: Connection::new().context("Couldn't connect to WM")?,
                events: Connection::new()
                    .context("Couldn't connect to WM")?
                    .subscribe([EventType::Window])
                    .context("Couldn't subscribe to events of type Window")?,
            })),
            _ => bail!("Not connecting to Sway or i3 as we've explicitly been asked not to"),
        }
    }

    fn get_windows_in_each_workspace(&mut self) -> Result<BTreeMap<String, Vec<Window>>> {
        self.connection
            .get_tree()
            .context("get_tree() failed")?
            .workspaces_in_node()
    }

    fn rename_workspace(&mut self, old: &str, new: &str) -> Result<()> {
        for result in self
            .connection
            .run_command(&format!("rename workspace \"{old}\" to \"{new}\"",))
            .context("Failed to rename the workspace")?
        {
            result.context("Failed to rename the workspace")?;
        }
        Ok(())
    }

    fn wait_for_event(&mut self) -> Result<()> {
        match self.events.next() {
            Some(Err(e)) => Err(anyhow!(e).context("Failed to receive next event")),
            None => bail!("Event stream ended"),
            _ => Ok(()),
        }
    }
}
