//! GPUI 0.2 has no semantic controls. Publish the launcher through the OS accessibility tree.
use super::*;
use accesskit::{Node, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate};
use accesskit_xplat::{Adapter, EventHandler, WindowEvent};
use dgl_core::agent::Agent;
use raw_window_handle::HasWindowHandle;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(super) enum Control {
    Menu,
    Run(Agent),
    Bypass(Agent),
    Option(Agent, usize),
    Settings,
}

struct Events(mpsc::UnboundedSender<WindowEvent>);
impl EventHandler for Events {
    fn handle_accesskit_event(&self, event: WindowEvent) {
        let _ = self.0.send(event);
    }
}

#[derive(Default, Clone)]
pub(super) struct LauncherAccessibility(Option<Rc<RefCell<Bridge>>>);
struct Bridge {
    adapter: Option<Adapter>,
    active: bool,
    tree: Option<TreeUpdate>,
    // Keep node identities stable while toggling, and retire them when the menu closes.
    ids: HashMap<Control, NodeId>,
    next_id: u64,
    nodes: Vec<(NodeId, Node)>,
    trigger: Option<Node>,
    focus: NodeId,
}

const ROOT: NodeId = NodeId(0);
const MENU: NodeId = NodeId(1);

impl LauncherAccessibility {
    pub(super) fn install(window: &mut Window, cx: &mut Context<Desktop>) -> Self {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return Self::default();
        };
        let (tx, mut events) = mpsc::unbounded_channel();
        let adapter = Adapter::with_combined_handler(handle.as_raw(), Arc::new(Events(tx)));
        let access = Self::new(Some(adapter));
        cx.spawn_in(window, async move |view, cx| {
            while let Some(event) = events.recv().await {
                if view
                    .update_in(cx, |s, window, cx| {
                        let Some(bridge) = &s.launch_access.0 else {
                            return;
                        };
                        let mut bridge = bridge.borrow_mut();
                        match event {
                            WindowEvent::InitialTreeRequested => {
                                bridge.tree = None;
                                bridge.active = true;
                            }
                            WindowEvent::AccessibilityDeactivated => {
                                bridge.active = false;
                                bridge.tree = None;
                            }
                            WindowEvent::ActionRequested(request) => {
                                if request.action != accesskit::Action::Click || s.dialog.is_some()
                                {
                                    return;
                                }
                                let control = if request.target_node == MENU {
                                    Some(Control::Menu)
                                } else {
                                    bridge.ids.iter().find_map(|(key, &id)| {
                                        (id == request.target_node).then_some(*key)
                                    })
                                };
                                bridge.focus = request.target_node;
                                drop(bridge);
                                if let Some(control) = control {
                                    s.activate_launcher_control(control, window, cx);
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        access
    }

    fn new(adapter: Option<Adapter>) -> Self {
        Self(Some(Rc::new(RefCell::new(Bridge {
            active: adapter.is_none(),
            adapter,
            tree: None,
            ids: HashMap::new(),
            next_id: 2,
            nodes: Vec::new(),
            trigger: None,
            focus: ROOT,
        }))))
    }

    pub(super) fn control(
        &self,
        element: Stateful<Div>,
        key: Control,
        label: &str,
        description: &str,
        checked: Option<bool>,
        enabled: bool,
    ) -> Stateful<Div> {
        let Some(bridge) = &self.0 else {
            return element;
        };
        if !bridge.borrow().active {
            return element;
        }
        let bridge = bridge.clone();
        let mut node = Node::new(if checked.is_some() {
            Role::CheckBox
        } else {
            Role::Button
        });
        node.set_label(label);
        if !description.is_empty() {
            node.set_description(description);
        }
        if let Some(checked) = checked {
            node.set_toggled(checked.into());
        }
        if enabled {
            node.add_action(accesskit::Action::Click);
        } else {
            node.set_disabled();
        }
        element.child(
            canvas(
                move |bounds, window, _| {
                    let mut bridge = bridge.borrow_mut();
                    if !bridge.active {
                        return;
                    }
                    let clipped = bounds.intersect(&window.content_mask().bounds);
                    let scale = window.scale_factor() as f64;
                    let coord = |n: Pixels| f32::from(n) as f64 * scale;
                    if clipped.size.width > px(0.) && clipped.size.height > px(0.) {
                        node.set_bounds(Rect::new(
                            coord(clipped.left()),
                            coord(clipped.top()),
                            coord(clipped.right()),
                            coord(clipped.bottom()),
                        ));
                    }
                    if key == Control::Menu {
                        bridge.trigger = Some(node);
                    } else {
                        let id = match bridge.ids.get(&key) {
                            Some(&id) => id,
                            None => {
                                let id = NodeId(bridge.next_id);
                                bridge.next_id += 1;
                                bridge.ids.insert(key, id);
                                id
                            }
                        };
                        bridge.nodes.push((id, node));
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
    }

    pub(super) fn finish(&self, root: Stateful<Div>, open: bool, available: bool) -> Stateful<Div> {
        let Some(bridge) = &self.0 else { return root };
        if !bridge.borrow().active {
            return root;
        }
        let bridge = bridge.clone();
        root.child(
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let mut bridge = bridge.borrow_mut();
                    if !bridge.active {
                        return;
                    }
                    let mut nodes = std::mem::take(&mut bridge.nodes);
                    if !open {
                        nodes.clear();
                        bridge.ids.clear();
                    }
                    if available && let Some(mut trigger) = bridge.trigger.clone() {
                        trigger.set_expanded(open);
                        nodes.insert(0, (MENU, trigger));
                    }
                    let mut root = Node::new(Role::Window);
                    root.set_label("DaGaLi");
                    root.set_children(nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>());
                    if !root.children().contains(&bridge.focus) {
                        bridge.focus = ROOT;
                    }
                    nodes.push((ROOT, root));
                    let tree = TreeUpdate {
                        tree_id: TreeId::ROOT,
                        tree: Some(TreeInfo::new(ROOT)),
                        nodes,
                        focus: bridge.focus,
                    };
                    if let Some(adapter) = &mut bridge.adapter {
                        let bounds = window.bounds();
                        let scale = window.scale_factor() as f64;
                        let physical = |p: Pixels| f32::from(p) as f64 * scale;
                        let origin = (physical(bounds.left()), physical(bounds.top()));
                        let rect = |s: Size<Pixels>| {
                            Rect::from_origin_size(origin, (physical(s.width), physical(s.height)))
                        };
                        adapter.set_window_bounds(rect(bounds.size), rect(window.viewport_size()));
                        adapter.set_focus(window.is_window_active());
                    }
                    if bridge.tree.as_ref() != Some(&tree) {
                        if let Some(adapter) = &mut bridge.adapter {
                            adapter.update_if_active(|| tree.clone());
                        }
                        bridge.tree = Some(tree);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, Control, LauncherAccessibility, Role};
    use gpui::{Modifiers, TestAppContext};

    #[gpui::test]
    fn launcher_tree_exposes_full_option_label_description_and_checked_state(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = crate::ui_tests::setup(cx);
        let access = LauncherAccessibility::new(None);
        let full = "Option with a name longer than the menu label";
        view.update(cx, |s, cx| {
            s.launch_access = access.clone();
            s.update_agent_launch(Agent::Claude, |config| {
                let opt = &mut config.options[0];
                opt.name = full.into();
                opt.description = "Detailed screen reader description".into();
            });
            cx.notify();
        });
        crate::ui_tests::redraw(cx);
        let center = cx.debug_bounds("cli-launcher").unwrap().center();
        cx.simulate_click(center, Modifiers::default());
        crate::ui_tests::redraw(cx);
        let check = |checked| {
            let state = access.0.as_ref().unwrap().borrow();
            let id = state.ids[&Control::Option(Agent::Claude, 0)];
            let node = &state
                .tree
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .find(|(i, _)| *i == id)
                .unwrap()
                .1;
            assert_eq!(node.role(), Role::CheckBox);
            assert_eq!(node.label(), Some(full));
            assert_eq!(
                node.description(),
                Some("Detailed screen reader description")
            );
            assert_eq!(
                node.toggled(),
                Some(if checked {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                })
            );
            assert!(node.bounds().is_some());
        };
        check(false);
        let bypass = || {
            let state = access.0.as_ref().unwrap().borrow();
            let id = state.ids[&Control::Bypass(Agent::Claude)];
            let node = &state
                .tree
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .find(|(key, _)| *key == id)
                .unwrap()
                .1;
            assert_eq!(node.role(), Role::CheckBox);
            assert_eq!(node.label(), Some("Bypass"));
            assert!(node.description().unwrap().contains("Claude Code"));
            node.toggled().unwrap()
        };
        assert_eq!(bypass(), accesskit::Toggled::False);
        let center = cx
            .debug_bounds("launch-Claude Code-bypass")
            .unwrap()
            .center();
        cx.simulate_click(center, Modifiers::default());
        crate::ui_tests::redraw(cx);
        assert_eq!(bypass(), accesskit::Toggled::True);
        check(false);
        let center = cx
            .debug_bounds("launch-Claude Code-option-0")
            .unwrap()
            .center();
        cx.simulate_click(center, Modifiers::default());
        crate::ui_tests::redraw(cx);
        check(true);
        cx.simulate_keystrokes("escape");
        crate::ui_tests::redraw(cx);
        assert!(access.0.as_ref().unwrap().borrow().ids.is_empty());
    }
}
