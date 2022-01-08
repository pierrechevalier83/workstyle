use futures_util::stream::StreamExt;
use std::collections::BTreeMap;

trait Node {
    fn is_workspace(&self) -> bool;
    fn is_window(&self) -> bool;
    fn name(&self) -> Option<String>;
    fn app_id(&self) -> Option<String>;
    fn window_properties_class(&self) -> Option<String>;
    fn windows_in_node(&self) -> Vec<Window>;
    fn workspaces_in_node(&self) -> Result<BTreeMap<String, Vec<Window>>, &'static str>;
}

impl Node for async_i3ipc::reply::Node {
    fn is_workspace(&self) -> bool {
        self.node_type == async_i3ipc::reply::NodeType::Workspace
    }
    fn is_window(&self) -> bool {
        matches!(
            self.node_type,
            async_i3ipc::reply::NodeType::Con | async_i3ipc::reply::NodeType::FloatingCon
        )
    }
    fn name(&self) -> Option<String> {
        self.name.clone()
    }
    fn app_id(&self) -> Option<String> {
        None
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
    fn workspaces_in_node(&self) -> Result<BTreeMap<String, Vec<Window>>, &'static str> {
        let mut res = BTreeMap::new();
        for node in &self.nodes {
            if node.is_workspace() {
                res.insert(
                    node.name().ok_or("Expected some node name")?,
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

impl Node for swayipc_async::Node {
    fn is_workspace(&self) -> bool {
        self.node_type == swayipc_async::NodeType::Workspace
    }
    fn is_window(&self) -> bool {
        matches!(
            self.node_type,
            swayipc_async::NodeType::Con | swayipc_async::NodeType::FloatingCon
        )
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
    fn workspaces_in_node(&self) -> Result<BTreeMap<String, Vec<Window>>, &'static str> {
        let mut res = BTreeMap::new();
        for node in &self.nodes {
            if node.is_workspace() {
                res.insert(
                    node.name().ok_or("Expected some node name")?,
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
    fn from_node(node: &dyn Node) -> Option<Self> {
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

enum Connection {
    I3(async_i3ipc::I3),
    Sway(swayipc_async::Connection),
}

pub enum Event {
    I3(async_i3ipc::event::Event),
    Sway(swayipc_async::Event),
}

pub enum EventStream {
    I3(async_i3ipc::stream::EventStream),
    Sway(swayipc_async::EventStream),
}

impl EventStream {
    pub async fn next(&mut self) -> Result<Event, &'static str> {
        match self {
            EventStream::I3(stream) => stream
                .next()
                .await
                .map(Event::I3)
                .map_err(|_| "I3: Failed to get next window event"),

            EventStream::Sway(stream) => stream
                .next()
                .await
                .ok_or("Sway: unexpectedly exhausted the event stream")?
                .map(Event::Sway)
                .map_err(|_| "Sway: Failed to get next window event"),
        }
    }
}

pub struct WindowManager {
    connection: Connection,
}

impl WindowManager {
    async fn connect_sway() -> Result<(Self, EventStream), &'static str> {
        let stream = swayipc_async::Connection::new()
            .await
            .map_err(|_| "Couldn't connect to sway")?
            .subscribe(&[swayipc_async::EventType::Window])
            .await
            .map_err(|_| "Couldn't subscribe to events of type Window with sway")?;
        Ok((
            Self {
                connection: Connection::Sway(
                    swayipc_async::Connection::new()
                        .await
                        .map_err(|_| "Couldn't connect to Sway")?,
                ),
            },
            EventStream::Sway(stream),
        ))
    }
    async fn connect_i3() -> Result<(Self, EventStream), &'static str> {
        let mut i3 = async_i3ipc::I3::connect()
            .await
            .map_err(|_| "Couldn't connect to I3")?;

        i3.subscribe(&[async_i3ipc::event::Subscribe::Window])
            .await
            .map_err(|_| "Couldn't subscribe to events of type Window with I3")?;
        let stream = i3.listen();
        Ok((
            Self {
                connection: Connection::I3(
                    async_i3ipc::I3::connect()
                        .await
                        .map_err(|_| "Couldn't connect to i3")?,
                ),
            },
            EventStream::I3(stream),
        ))
    }
    pub async fn connect() -> Result<(Self, EventStream), &'static str> {
        use sysinfo::{ProcessExt, System, SystemExt};

        let s = System::new_all();
        let is_sway = s.processes().values().any(|x| x.name() == "sway");
        let is_i3 = s.processes().values().any(|x| x.name() == "i3");
        if is_sway {
            let ret = Self::connect_sway().await?;
            log::info!("Connected to sway");
            Ok(ret)
        } else if is_i3 {
            let ret = Self::connect_i3().await?;
            log::info!("Connected to i3");
            Ok(ret)
        } else {
            log::info!("Neither sway nor i3 was running");
            Err("Couldn't connect to sway or i3 wm")
        }
    }
    pub async fn get_windows_in_each_workspace(
        &mut self,
    ) -> Result<BTreeMap<String, Vec<Window>>, &'static str> {
        match &mut self.connection {
            Connection::I3(connection) => connection
                .get_tree()
                .await
                .map_err(|_| "Failed to get_tree with i3")?
                .workspaces_in_node(),
            Connection::Sway(connection) => connection
                .get_tree()
                .await
                .map_err(|_| "Failed to get_tree with sway")?
                .workspaces_in_node(),
        }
    }

    pub async fn rename_workspaces(
        &mut self,
        new_names: BTreeMap<String, String>,
    ) -> Result<(), &'static str> {
        match &mut self.connection {
            Connection::I3(connection) => {
                for workspace in connection
                    .get_workspaces()
                    .await
                    .map_err(|_| "Failed to get_workspaces with i3")?
                    .iter()
                {
                    connection
                        .run_command(&format!(
                            "rename workspace \"{}\" to \"{}\"",
                            &workspace.name,
                            &new_names.get(&workspace.name).unwrap_or(&workspace.name)
                        ))
                        .await
                        .map_err(|_| "Failed to run_command with i3")?;
                }
                Ok(())
            }
            Connection::Sway(connection) => {
                for workspace in connection
                    .get_workspaces()
                    .await
                    .map_err(|_| "Failed to get_workspaces with sway")?
                    .iter()
                {
                    connection
                        .run_command(&format!(
                            "rename workspace \"{}\" to \"{}\"",
                            &workspace.name,
                            &new_names.get(&workspace.name).unwrap_or(&workspace.name)
                        ))
                        .await
                        .map_err(|_| "Failed to run_command with sway")?;
                }
                Ok(())
            }
        }
    }
}
