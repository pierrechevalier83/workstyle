use i3ipc::{
    reply::{Node, NodeType},
    I3Connection, I3EventListener, Subscription,
};
use std::collections::BTreeMap;

/// Recursively find all windows names in this node
fn windows_in_node(node: &Node) -> Vec<Option<String>> {
    let mut res = Vec::new();
    for node in node.nodes.clone() {
        if node.nodetype == NodeType::Con {
            res.push(node.name)
        } else {
            let child_windows = windows_in_node(&node);
            for window in child_windows {
                res.push(window)
            }
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
    icon_mappings: &[(String, char)],
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

fn icon_mappings() -> Vec<(String, char)> {
    let content = String::from_utf8(include_bytes!("icon_mappings.txt").to_vec()).unwrap();
    content
        .split("\n")
        .filter_map(|s| {
            let mut split = s.split(": ");
            split.next().and_then(|name| {
                split
                    .next()
                    .and_then(|icon| icon.chars().next())
                    .map(|icon| (name.to_string(), icon))
            })
        })
        .collect()
}

fn pretty_window(window: &String, icon_mappings: &[(String, char)]) -> char {
    for (name, icon) in icon_mappings {
        if window.to_lowercase().contains(name) {
            return *icon;
        }
    }
    println!("Couldn't identify window: {}", window);
    ''
}

fn pretty_windows(windows: &Vec<Option<String>>, icon_mappings: &[(String, char)]) -> String {
    let mut s = String::new();
    for window in windows {
        if let Some(window) = window {
            s.push(pretty_window(window, icon_mappings));
            s.push(' ');
        }
    }
    s
}

fn main() {
    let mut wm = I3Connection::connect().unwrap();
    let mut listener = I3EventListener::connect().unwrap();
    listener.subscribe(&[Subscription::Window]);
    let icon_mappings = icon_mappings();
    listener.listen().for_each(|_| {
        let tree = wm.get_tree().unwrap();
        let workspaces = workspaces_in_node(&tree);
        rename_workspaces(&mut wm, &workspaces, &icon_mappings);
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
}
