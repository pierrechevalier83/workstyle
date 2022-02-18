use anyhow::Result;
use std::collections::HashMap;
use swayipc::{Connection, EventStream, Node, NodeType};

trait NodeExt {
    fn is_window(&self) -> bool;
    fn window_properties_class(&self) -> Option<String>;
    fn windows_in_node(&self) -> Vec<Window>;
    fn workspaces_in_node(&self) -> Result<HashMap<String, Vec<Window>>>;
}

impl NodeExt for Node {
    fn is_window(&self) -> bool {
        matches!(
            self.node_type,
            swayipc::NodeType::Con | swayipc::NodeType::FloatingCon
        )
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
    fn workspaces_in_node(&self) -> Result<HashMap<String, Vec<Window>>> {
        let mut res = HashMap::new();
        for node in &self.nodes {
            if node.node_type == NodeType::Workspace {
                res.insert(
                    node.name
                        .clone()
                        .ok_or(anyhow!("Expected some node name"))?,
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
            let name = node.name.clone();
            let app_id = node.app_id.clone();
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
            .map_or(false, |s| s.to_lowercase().contains(pattern))
            || self
                .app_id
                .as_ref()
                .map_or(false, |s| s.to_lowercase().contains(pattern))
            || self
                .window_properties_class
                .as_ref()
                .map_or(false, |s| s.to_lowercase().contains(pattern))
    }
}

pub struct WindowManager {
    connection: Connection,
}

impl WindowManager {
    pub fn connect() -> Result<(Self, EventStream)> {
        Ok((
            Self {
                connection: Connection::new()?,
            },
            Connection::new()?.subscribe(&[swayipc::EventType::Window])?,
        ))
    }

    pub fn get_windows_in_each_workspace(&mut self) -> Result<HashMap<String, Vec<Window>>> {
        self.connection.get_tree()?.workspaces_in_node()
    }

    pub fn rename_workspace(&mut self, old: &str, new: &str) -> Result<()> {
        self.connection
            .run_command(&format!("rename workspace \"{old}\" to \"{new}\"",))?;
        Ok(())
    }
}
