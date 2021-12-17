use std::collections::BTreeMap;

trait Node {
    fn is_workspace(&self) -> bool;
    fn is_window(&self) -> bool;
    fn name(&self) -> Option<String>;
    fn app_id(&self) -> Option<String>;
    fn window_properties_class(&self) -> Option<String>;
    fn windows_in_node(&self) -> Vec<Window>;
    fn workspaces_in_node(&self) -> BTreeMap<String, Vec<Window>>;
}

impl Node for i3ipc::reply::Node {
    fn is_workspace(&self) -> bool {
        self.nodetype == i3ipc::reply::NodeType::Workspace
    }
    fn is_window(&self) -> bool {
        match self.nodetype {
            i3ipc::reply::NodeType::Con | i3ipc::reply::NodeType::FloatingCon => true,
            _ => false,
        }
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
            .and_then(|prop| prop.get(&i3ipc::reply::WindowProperty::Class).cloned())
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
    fn workspaces_in_node(&self) -> BTreeMap<String, Vec<Window>> {
        let mut res = BTreeMap::new();
        for node in &self.nodes {
            if node.is_workspace() {
                res.insert(node.name().unwrap(), node.windows_in_node());
            } else {
                let workspaces = node.workspaces_in_node();
                for (k, v) in workspaces {
                    res.insert(k, v);
                }
            }
        }
        res
    }
}

impl Node for swayipc::reply::Node {
    fn is_workspace(&self) -> bool {
        self.node_type == swayipc::reply::NodeType::Workspace
    }
    fn is_window(&self) -> bool {
        match self.node_type {
            swayipc::reply::NodeType::Con | swayipc::reply::NodeType::FloatingCon => true,
            _ => false,
        }
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
    fn workspaces_in_node(&self) -> BTreeMap<String, Vec<Window>> {
        let mut res = BTreeMap::new();
        for node in &self.nodes {
            if node.is_workspace() {
                res.insert(node.name().unwrap(), node.windows_in_node());
            } else {
                let workspaces = node.workspaces_in_node();
                for (k, v) in workspaces {
                    res.insert(k, v);
                }
            }
        }
        res
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
    I3(i3ipc::I3Connection),
    Sway(swayipc::Connection),
}

pub enum Event {
    I3(i3ipc::event::Event),
    Sway(swayipc::reply::Event),
}

pub enum EventListener {
    I3(i3ipc::I3EventListener),
    Sway(swayipc::EventIterator),
}

impl EventListener {
    pub fn window_events<'a>(&'a mut self) -> Box<dyn Iterator<Item = Event> + 'a> {
        match self {
            EventListener::I3(listener) => Box::new(
                listener
                    .listen()
                    .filter_map(|event| event.ok())
                    .map(|i3_event| Event::I3(i3_event)),
            ),
            EventListener::Sway(iterator) => Box::new(
                iterator
                    .filter_map(|event| event.ok())
                    .map(|sway_event| Event::Sway(sway_event)),
            ),
        }
    }
}

pub struct WindowManager {
    connection: Connection,
}

impl WindowManager {
    pub fn connect() -> (Self, EventListener) {
        if swayipc::Connection::new()
            .map(|mut connection| connection.get_tree().is_ok())
            .unwrap_or(false)
        {
            let listener = swayipc::Connection::new()
                .unwrap()
                .subscribe(&[swayipc::EventType::Window])
                .unwrap();
            (
                Self {
                    connection: Connection::Sway(swayipc::Connection::new().unwrap()),
                },
                EventListener::Sway(listener),
            )
        } else if i3ipc::I3Connection::connect()
            .map(|mut connection| connection.get_tree().is_ok())
            .unwrap_or(false)
        {
            let mut listener = i3ipc::I3EventListener::connect().unwrap();
            listener.subscribe(&[i3ipc::Subscription::Window]).unwrap();
            (
                Self {
                    connection: Connection::I3(i3ipc::I3Connection::connect().unwrap()),
                },
                EventListener::I3(listener),
            )
        } else {
            panic!("Error, failed to connect to both sway and i3");
        }
    }
    pub fn get_windows_in_each_workspace(&mut self) -> BTreeMap<String, Vec<Window>> {
        match &mut self.connection {
            Connection::I3(connection) => connection.get_tree().unwrap().workspaces_in_node(),
            Connection::Sway(connection) => connection.get_tree().unwrap().workspaces_in_node(),
        }
    }

    pub fn rename_workspaces(&mut self, new_names: BTreeMap<String, String>) {
        match &mut self.connection {
            Connection::I3(connection) => connection
                .get_workspaces()
                .unwrap()
                .workspaces
                .iter()
                .for_each(|workspace| {
                    connection
                        .run_command(&format!(
                            "rename workspace \"{}\" to \"{}\"",
                            &workspace.name, &new_names.get(&workspace.name).unwrap_or(&workspace.name)
                        ))
                        .unwrap();
                }),
            Connection::Sway(connection) => {
                connection
                    .get_workspaces()
                    .unwrap()
                    .iter()
                    .for_each(|workspace| {
                        connection
                            .run_command(&format!(
                                "rename workspace \"{}\" to \"{}\"",
                                &workspace.name, &new_names.get(&workspace.name).unwrap_or(&workspace.name)
                            ))
                            .unwrap();
                    })
            }
        }
    }
}
