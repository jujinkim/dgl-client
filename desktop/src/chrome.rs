//! Window chrome as cached views. Terminal output repaints the window at the frame rate;
//! GPUI then renders the root again, but a cached view that was not notified reuses its
//! previous layout and paint. Each part renders from `Desktop` and repaints whenever
//! `Desktop` notifies, so the parts cost nothing while only a terminal changes.
use crate::{
    Desktop,
    workspace::{Group, GroupId},
};
use gpui::{prelude::*, *};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Part {
    Sidebar,
    GroupHeader(GroupId),
    PaneToolbar(GroupId),
    InputTools,
    StatusBar,
}

struct PartView {
    desktop: WeakEntity<Desktop>,
    part: Part,
    #[cfg(test)]
    renders: std::rc::Rc<std::cell::Cell<usize>>,
}
impl Render for PartView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let part = self.part;
        #[cfg(test)]
        {
            self.renders.set(self.renders.get() + 1);
        }
        self.desktop
            .update(cx, |desktop, cx| match part {
                Part::Sidebar => desktop.render_sidebar(cx).into_any_element(),
                Part::GroupHeader(id) => desktop.render_group_header(id, cx).into_any_element(),
                Part::PaneToolbar(id) => desktop.render_group_pane_toolbar(id, cx),
                Part::InputTools => desktop.render_input_tools(cx).into_any_element(),
                Part::StatusBar => desktop.render_status_bar(window, cx).into_any_element(),
            })
            .unwrap_or_else(|_| Empty.into_any_element())
    }
}

#[derive(Default)]
pub(crate) struct Chrome {
    views: HashMap<Part, Entity<PartView>>,
    /// Set while rendering when something the parts draw changed without a notification
    /// (the palette follows the window appearance): this frame renders them afresh.
    pub stale: bool,
    /// The active terminal's sticky modifiers the shortcut bar last drew.
    pub modifiers: dgl_core::input::Modifiers,
    /// The height each group's pane toolbar last laid out with. The toolbar wraps, and
    /// a cached view needs its size before layout.
    pub toolbar_heights: HashMap<GroupId, Pixels>,
    #[cfg(test)]
    renders: HashMap<Part, std::rc::Rc<std::cell::Cell<usize>>>,
}
impl Chrome {
    /// Drops the views of groups that no longer exist.
    pub fn retain_groups(&mut self, groups: &BTreeMap<GroupId, Group>) {
        self.toolbar_heights.retain(|id, _| groups.contains_key(id));
        #[cfg(test)]
        self.renders.retain(|part, _| match part {
            Part::GroupHeader(id) | Part::PaneToolbar(id) => groups.contains_key(id),
            _ => true,
        });
        self.views.retain(|part, _| match part {
            Part::GroupHeader(id) | Part::PaneToolbar(id) => groups.contains_key(id),
            Part::Sidebar | Part::InputTools | Part::StatusBar => true,
        });
    }
}
#[cfg(test)]
impl Chrome {
    /// How often `part` rendered; 0 when it has no view.
    pub fn renders(&self, part: Part) -> usize {
        self.renders.get(&part).map_or(0, |count| count.get())
    }
}
impl Desktop {
    /// The element of `part`; `style` is its root layout, which a reused view keeps.
    /// `fresh` renders it anew this frame, for a change that did not notify `Desktop`.
    pub(crate) fn chrome(
        &mut self,
        part: Part,
        style: StyleRefinement,
        fresh: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let desktop = cx.entity();
        #[cfg(test)]
        let renders = self.chrome.renders.entry(part).or_default().clone();
        let view = self
            .chrome
            .views
            .entry(part)
            .or_insert_with(|| {
                cx.new(|cx| {
                    cx.observe(&desktop, |_, _, cx| cx.notify()).detach();
                    PartView {
                        desktop: desktop.downgrade(),
                        part,
                        #[cfg(test)]
                        renders,
                    }
                })
            })
            .clone();
        if fresh || self.chrome.stale {
            view.into_any_element()
        } else {
            AnyView::from(view).cached(style).into_any_element()
        }
    }
}
