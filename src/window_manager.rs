use log::{info, debug};

use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;
use swayipc::{Connection, EventStream, EventType, Node, NodeType, Event, WorkspaceChange, WindowChange};

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
    name: Option<String>,
    app_id: Option<String>,
    window_properties_class: Option<String>,
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

pub struct WindowManager {
    connection: Connection,
    events: EventStream,
}

fn should_rename_after_event(event: Event) -> Result<()> {
    match event {
        Event::Workspace(boxed_workspace_event) => {
            match (*boxed_workspace_event).change {
                ch @ WorkspaceChange::Focus => {
                    // Triggering on Init triggers an extra focus after
                    // the initial focus.
                    // Simply running a rename on every Workspace Focus
                    // prevents this irritating visual artifact,
                    // at the cost of running renames more often.
                    info!("Renaming on WorkspaceEvent: {:#?}", ch);
                    Ok(())
                },
                ch @ _ => {
                    debug!("Rejected Rename for WorkspaceEvent: {:#?}", ch);
                    Err(anyhow!("bad_ev").context("Only certain WorkspaceChange events should trigger rename"))
                },
            }
        }
        Event::Window(boxed_window_event) => {
            match (*boxed_window_event).change {
                ch @ (
                    WindowChange::New |
                    WindowChange::Close |
                    WindowChange::Title |
                    WindowChange::Move |
                    WindowChange::Urgent |
                    WindowChange::Mark
                ) => {
                    info!("Renaming on WindowEvent: {:#?}", ch);
                    Ok(())
                },
                ch @ _ => {
                    debug!("Rejected Rename for WindowEvent: {:#?}", ch);
                    Err(anyhow!("bad_ev").context("Only certain WindowChange events should trigger rename"))
                },
            }
        }
        _ => Err(anyhow!("bad_ev").context("Only certain Workspace and Window *Change events trigger a rename")),
    }
}

impl WindowManager {
    pub fn connect() -> Result<Self> {
        Ok(Self {
            connection: Connection::new().context("Couldn't connect to WM")?,
            events: Connection::new()
                .context("Couldn't connect to WM")?
                .subscribe(&[EventType::Window, EventType::Workspace])
                .context("Couldn't subscribe to events of type Window or Workspace")?,
        })
    }

    pub fn get_windows_in_each_workspace(&mut self) -> Result<BTreeMap<String, Vec<Window>>> {
        self.connection
            .get_tree()
            .context("get_tree() failed")?
            .workspaces_in_node()
    }

    pub fn rename_workspace(&mut self, old: &str, new: &str) -> Result<()> {
        for result in self
            .connection
            .run_command(&format!("rename workspace \"{old}\" to \"{new}\"",))
            .context("Failed to rename the workspace")?
        {
            result.context("Failed to rename the workspace")?;
        }
        Ok(())
    }

    pub fn wait_for_event(&mut self) -> Result<()> {
        match self.events.next() {
            Some(Err(e)) => Err(anyhow!(e).context("Failed to receive next event")),
            None => bail!("Event stream ended"),
            Some(event) => {
                // Scan the kind of event to get the relevant Result
                should_rename_after_event(event.unwrap())
            },
        }
    }
}
