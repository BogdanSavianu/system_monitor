use dioxus::prelude::*;

use crate::app::gui::view_models::ProcessHierarchyNodeViewModel;

pub fn render_process_tree_view(
    roots: &[ProcessHierarchyNodeViewModel],
    expanded: &std::collections::HashSet<u32>,
    on_toggle_node: EventHandler<u32>,
) -> Element {
    rsx! {
        div {
            class: "list-panel",
            h3 { "Process tree" }
            p {
                class: "detected-leaks-subtitle",
                "Click the arrow to expand/collapse child processes"
            }

            if roots.is_empty() {
                p { class: "detected-leaks-empty", "No process tree data yet" }
            } else {
                div { class: "process-tree",
                    for node in roots {
                        {render_tree_node(node, expanded, on_toggle_node, 0)}
                    }
                }
            }
        }
    }
}

fn render_tree_node(
    node: &ProcessHierarchyNodeViewModel,
    expanded: &std::collections::HashSet<u32>,
    on_toggle_node: EventHandler<u32>,
    depth: usize,
) -> Element {
    let node_pid = node.pid;
    let has_children = !node.children.is_empty();
    let is_expanded = expanded.contains(&node.pid);
    let padding_left = format!("{}px", depth * 18);

    rsx! {
        div {
            class: "process-tree-node",
            style: "padding-left: {padding_left};",

            if has_children {
                button {
                    class: "tree-toggle-btn",
                    onclick: move |_| on_toggle_node.call(node_pid),
                    if is_expanded { "v" } else { ">" }
                }
            } else {
                span { class: "tree-leaf-marker", "-" }
            }

            span { class: "tree-node-name", "{node.name}" }
            span { class: "tree-node-pid", "(pid: {node.pid}, ppid: {node.ppid})" }
        }

        if has_children && is_expanded {
            for child in &node.children {
                {render_tree_node(child, expanded, on_toggle_node, depth + 1)}
            }
        }
    }
}
