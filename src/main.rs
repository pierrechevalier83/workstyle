mod config;

use std::collections::BTreeMap;
use structopt::StructOpt;
use swayipc::{
    reply::{Node, NodeType},
    Connection, EventType,
};

#[derive(StructOpt)]
#[structopt(
    name = "workstyle",
    about = "\nWorkspaces with style!\n\nThis program will dynamically rename your workspaces to indicate which programs are running in each workspace. It uses the i3 ipc protocol, which makes it compatible with sway and i3.\n\nBy default, each program is mapped to a unicode character for concision.\n\nThe short description of each program is configurable. In the absence of a config file, one will be generated automatically.\nSee ${XDG_CONFIG_HOME}/workstyle/config.yml for  details."
)]
struct Options {}

#[derive(Debug)]
struct Window {
    name: Option<String>,
    app_id: Option<String>,
    window_properties_class: Option<String>,
}

impl Window {
    fn from_node(node: &Node) -> Option<Self> {
        match node.node_type {
            NodeType::Con | NodeType::FloatingCon => {
                let name = node.name.clone();
                let app_id = node.app_id.clone();
                let window_properties_class = node
                    .window_properties
                    .as_ref()
                    .and_then(|window_properties| window_properties.class.clone());
                if name.is_some() || app_id.is_some() || window_properties_class.is_some() {
                    Some(Self {
                        name,
                        app_id,
                        window_properties_class,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }
    fn matches(&self, pattern: &str) -> bool {
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

/// Recursively find all windows names in this node
fn windows_in_node(node: &Node) -> Vec<Window> {
    let mut res = Vec::new();
    for node in node.nodes.iter().chain(node.floating_nodes.iter()) {
        res.extend(windows_in_node(node));
        if let Some(window) = Window::from_node(&node) {
            res.push(window);
        }
    }
    res
}

/// Recursively find all workspaces in this node and the list of open windows for each of these
/// workspaces
fn workspaces_in_node(node: &Node) -> BTreeMap<String, Vec<Window>> {
    let mut res = BTreeMap::new();
    for node in &node.nodes {
        if node.node_type == NodeType::Workspace {
            let name = node.name.clone().unwrap();
            res.insert(name, windows_in_node(&node));
        } else {
            let workspaces = workspaces_in_node(&node);
            for (k, v) in workspaces {
                res.insert(k, v);
            }
        }
    }
    res
}

fn rename_workspaces(
    wm: &mut Connection,
    workspaces: &BTreeMap<String, Vec<Window>>,
    icon_mappings: &[(String, String)],
    fallback_icon: &String,
) {
    wm.get_workspaces()
        .unwrap()
        .iter()
        .map(|workspace| {
            let name = workspace.name.clone();
            let new_name = pretty_windows(&workspaces[&name], icon_mappings, fallback_icon);
            let new_name = if new_name == "" {
                format!("{}", workspace.num)
            } else {
                format!("{}: {}", workspace.num, new_name)
            };
            format!("rename workspace \"{}\" to \"{}\"", &name, &new_name)
        })
        .for_each(|command| {
            wm.run_command(&command).unwrap();
        })
}

fn pretty_window(
    window: &Window,
    icon_mappings: &[(String, String)],
    fallback_icon: &String,
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
    windows: &Vec<Window>,
    icon_mappings: &[(String, String)],
    fallback_icon: &String,
) -> String {
    let mut s = String::new();
    for window in windows {
        s.push_str(&pretty_window(window, icon_mappings, fallback_icon));
        s.push(' ');
    }
    s
}

fn main() {
    pretty_env_logger::init();
    let _ = Options::from_args();
    let mut wm = Connection::new().unwrap();
    let on_window_events = Connection::new()
        .unwrap()
        .subscribe(&[EventType::Window])
        .unwrap();
    let config_file = config::generate_config_file_if_absent();
    on_window_events.for_each(|_| {
        let fallback_icon = config::get_fallback_icon(&config_file);
        let icon_mappings = config::get_icon_mappings(&config_file);
        let tree = wm.get_tree().unwrap();
        let workspaces = workspaces_in_node(&tree);
        rename_workspaces(&mut wm, &workspaces, &icon_mappings, &fallback_icon);
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
}
