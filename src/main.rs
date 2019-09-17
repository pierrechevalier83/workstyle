mod config;

use i3ipc::{
    reply::{Node, NodeType},
    I3Connection, I3EventListener, Subscription,
};
use std::collections::BTreeMap;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    about = "\nSway workspace names.\n\nThis program will dynamically rename your workspaces to indicate which programs are running in each workspace. It uses the i3 ipc protocol, so should work on i3 or sway.\n\nBy default, each program is mapped to a unicode character for concision.\n\nThe short description of each program is configurable. In the absence of a config file, one will be generated automatically.\nSee ${XDG_CONFIG_HOME}/sway_workspace_names/config.yml for  details."
)]
struct Options {}

/// Recursively find all windows names in this node
fn windows_in_node(node: &Node) -> Vec<Option<String>> {
    let mut res = Vec::new();
    for node in node
        .nodes
        .clone()
        .iter()
        .chain(node.floating_nodes.clone().iter())
    {
        res.extend(windows_in_node(node));
        match node.nodetype {
            NodeType::Con | NodeType::FloatingCon => res.push(node.name.clone()),
            _ => (),
        }
    }
    res
}

/// Recursively find all workspaces in this node and the list of open windows for each of these
/// workspaces
fn workspaces_in_node(node: &Node) -> BTreeMap<String, Vec<Option<String>>> {
    let mut res = BTreeMap::new();
    for node in node.nodes.clone() {
        if node.nodetype == NodeType::Workspace {
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
    wm: &mut I3Connection,
    workspaces: &BTreeMap<String, Vec<Option<String>>>,
    icon_mappings: &[(String, String)],
) {
    wm.get_workspaces()
        .unwrap()
        .workspaces
        .iter()
        .map(|workspace| {
            let name = workspace.name.clone();
            let new_name = pretty_windows(&workspaces[&name], icon_mappings);
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

fn pretty_window(window: &String, icon_mappings: &[(String, String)]) -> String {
    for (name, icon) in icon_mappings {
        if window.to_lowercase().contains(name) {
            return icon.clone();
        }
    }
    println!("Couldn't identify window: {}", window);
    "".into()
}

fn pretty_windows(windows: &Vec<Option<String>>, icon_mappings: &[(String, String)]) -> String {
    let mut s = String::new();
    for window in windows {
        if let Some(window) = window {
            s.push_str(&pretty_window(window, icon_mappings));
            s.push(' ');
        }
    }
    s
}

fn main() {
    let _ = Options::from_args();
    let mut wm = I3Connection::connect().unwrap();
    let mut listener = I3EventListener::connect().unwrap();
    listener.subscribe(&[Subscription::Window]).unwrap();
    let config_file = config::generate_config_file_if_absent();
    listener.listen().for_each(|_| {
        let icon_mappings = config::get_icon_mappings(&config_file);
        let tree = wm.get_tree().unwrap();
        let workspaces = workspaces_in_node(&tree);
        rename_workspaces(&mut wm, &workspaces, &icon_mappings);
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
}
