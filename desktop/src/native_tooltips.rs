//! OS-owned tooltip presentation. GPUI only supplies clipped, hit-tested regions.
use gpui::{prelude::*, *};
#[cfg(not(test))]
use raw_window_handle::HasWindowHandle;
use raw_window_handle::RawWindowHandle;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use self::macos::Tooltips;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use self::windows::Tooltips;

#[derive(Clone, Debug, PartialEq)]
struct Region {
    id: u64,
    bounds: Bounds<Pixels>,
    text: String,
    scale: f32,
}
struct Candidate {
    region: Region,
    hitbox: Hitbox,
}
#[derive(Default)]
struct Registry(HashMap<WindowId, Weak<RefCell<Manager>>>);
impl Global for Registry {}

struct Manager {
    regions: Vec<Candidate>,
    current: Option<Region>,
    suspended: bool,
    native: Option<Tooltips>,
}
impl Manager {
    fn new(window: &Window) -> Self {
        // GPUI's test platform deliberately panics when asked for a raw handle.
        #[cfg(not(test))]
        let raw = HasWindowHandle::window_handle(window)
            .ok()
            .map(|h| h.as_raw());
        #[cfg(test)]
        let raw: Option<RawWindowHandle> = None;
        #[cfg(test)]
        let _ = window;
        Self {
            regions: vec![],
            current: None,
            suspended: false,
            native: match raw {
                #[cfg(target_os = "macos")]
                Some(RawWindowHandle::AppKit(h)) => Tooltips::new(h.ns_view.as_ptr()),
                #[cfg(target_os = "windows")]
                Some(RawWindowHandle::Win32(h)) => Tooltips::new(h.hwnd.get()),
                _ => None,
            },
        }
    }
    fn sync(&mut self, window: &Window) {
        // Only the topmost hovered region is handed to the OS. In particular,
        // modal/launcher occlusion cannot expose a background native tooltip.
        let next = (window.is_window_active() && !self.suspended)
            .then(|| {
                self.regions
                    .iter()
                    .rev()
                    .find(|r| r.hitbox.is_hovered(window))
                    .map(|r| r.region.clone())
            })
            .flatten();
        self.set(next);
    }
    fn suspend(&mut self) {
        self.suspended = true;
        self.set(None);
    }
    fn set(&mut self, next: Option<Region>) {
        if self.current == next {
            return;
        }
        if let Some(native) = &mut self.native {
            native.set(next.as_ref());
        }
        self.current = next;
    }
}

fn clipped(
    bounds: Bounds<Pixels>,
    mask: Bounds<Pixels>,
    viewport: Size<Pixels>,
) -> Option<Bounds<Pixels>> {
    let bounds = bounds
        .intersect(&mask)
        .intersect(&Bounds::new(point(px(0.), px(0.)), viewport));
    (bounds.size.width > px(0.) && bounds.size.height > px(0.)).then_some(bounds)
}

pub trait NativeTooltip: Sized {
    fn native_tooltip(self, text: impl Into<String>) -> Self;
}
impl NativeTooltip for Stateful<Div> {
    fn native_tooltip(self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.child(
            canvas(
                move |bounds, window, cx| {
                    let Some(manager) = cx
                        .try_global::<Registry>()
                        .and_then(|r| r.0.get(&Window::window_handle(window).window_id()))
                        .and_then(Weak::upgrade)
                    else {
                        return;
                    };
                    let Some(bounds) =
                        clipped(bounds, window.content_mask().bounds, window.viewport_size())
                    else {
                        return;
                    };
                    if text.is_empty() {
                        return;
                    }
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    let mut manager = manager.borrow_mut();
                    let id = manager.regions.len() as u64 + 1;
                    manager.regions.push(Candidate {
                        region: Region {
                            id,
                            bounds,
                            text,
                            scale: window.scale_factor(),
                        },
                        hitbox,
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
    }
}

/// The element state owns one native manager for exactly this window's lifetime.
pub fn root(child: impl IntoElement) -> impl IntoElement {
    TooltipRoot(child.into_any_element())
}
struct TooltipRoot(AnyElement);
impl IntoElement for TooltipRoot {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TooltipRoot {
    type RequestLayoutState = Rc<RefCell<Manager>>;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        Some("native-tooltips".into())
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let manager = window.with_element_state(
            id.unwrap(),
            |previous: Option<Rc<RefCell<Manager>>>, window| {
                let manager =
                    previous.unwrap_or_else(|| Rc::new(RefCell::new(Manager::new(window))));
                (manager.clone(), manager)
            },
        );
        let registry = cx.default_global::<Registry>();
        registry.0.retain(|_, weak| weak.strong_count() > 0);
        registry.0.insert(
            Window::window_handle(window).window_id(),
            Rc::downgrade(&manager),
        );
        (self.0.request_layout(window, cx), manager)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        manager: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        manager.borrow_mut().regions.clear();
        self.0.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        manager: &mut Self::RequestLayoutState,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.paint(window, cx);
        if cx.has_active_drag() {
            manager.borrow_mut().suspend();
        } else {
            manager.borrow_mut().sync(window);
        }
        let moved = manager.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Capture {
                if event.pressed_button.is_some() {
                    moved.borrow_mut().suspend();
                    return;
                }
                moved.borrow_mut().suspended = false;
                moved.borrow_mut().sync(window);
                #[cfg(target_os = "windows")]
                if let Some(native) = &moved.borrow().native {
                    native.relay_move(window.mouse_position(), window.scale_factor());
                }
            }
        });
        let pressed = manager.clone();
        window.on_mouse_event(move |_: &MouseDownEvent, phase, _, _| {
            if phase == DispatchPhase::Capture {
                pressed.borrow_mut().suspend();
            }
        });
        let scrolled = manager.clone();
        window.on_mouse_event(move |_: &ScrollWheelEvent, phase, _, _| {
            if phase == DispatchPhase::Capture {
                scrolled.borrow_mut().suspend();
            }
        });
        let exited = manager.clone();
        window.on_mouse_event(move |_: &MouseExitEvent, phase, _, _| {
            if phase == DispatchPhase::Capture {
                exited.borrow_mut().suspend();
            }
        });
        let keyed = manager.clone();
        window.on_key_event(move |_: &KeyDownEvent, phase, _, _| {
            if phase == DispatchPhase::Capture {
                keyed.borrow_mut().suspend();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeTooltip, Registry, clipped, root};
    use gpui::{
        Bounds, Context, IntoElement, MouseMoveEvent, Render, TestAppContext, VisualTestContext,
        Window, div, point, prelude::*, px, size,
    };
    #[test]
    fn regions_clip_to_scroll_mask_and_viewport_and_reject_hidden_elements() {
        let b = |x, y, w, h| Bounds::new(point(px(x), px(y)), size(px(w), px(h)));
        assert_eq!(
            clipped(
                b(10., 10., 100., 40.),
                b(0., 20., 90., 80.),
                size(px(80.), px(60.))
            ),
            Some(b(10., 20., 70., 30.))
        );
        assert_eq!(
            clipped(
                b(10., -50., 100., 40.),
                b(0., 0., 100., 100.),
                size(px(100.), px(100.))
            ),
            None
        );
        assert_eq!(
            clipped(
                b(100., 0., 100., 40.),
                b(0., 0., 100., 100.),
                size(px(100.), px(100.))
            ),
            None
        );
    }

    struct Fixture {
        show: bool,
        covered: bool,
        width: f32,
        scroll: gpui::ScrollHandle,
    }
    impl Render for Fixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            root(
                div()
                    .size_full()
                    .relative()
                    .child(
                        div()
                            .id("tip-viewport")
                            .w(px(self.width))
                            .h(px(60.))
                            .overflow_y_scroll()
                            .overflow_x_hidden()
                            .track_scroll(&self.scroll)
                            .when(self.show, |d| {
                                d.child(
                                    div()
                                        .id("tip-target")
                                        .w(px(200.))
                                        .h(px(100.))
                                        .native_tooltip("한국어 · English\n%1 / @2 / $3"),
                                )
                                .child(
                                    div()
                                        .id("tip-second")
                                        .w(px(200.))
                                        .h(px(100.))
                                        .native_tooltip("두 번째 항목"),
                                )
                            }),
                    )
                    .when(self.covered, |d| {
                        d.child(div().id("cover").absolute().inset_0().occlude())
                    }),
            )
        }
    }
    fn redraw(ui: &mut VisualTestContext) {
        ui.update(|w, _| w.refresh());
        ui.run_until_parked();
    }
    fn snapshot(ui: &mut VisualTestContext) -> (usize, Option<super::Region>) {
        ui.update(|w, cx| {
            let manager = cx.global::<Registry>().0[&Window::window_handle(w).window_id()]
                .upgrade()
                .unwrap();
            let manager = manager.borrow();
            (manager.regions.len(), manager.current.clone())
        })
    }
    #[gpui::test]
    fn native_regions_follow_clipping_resize_occlusion_removal_and_window_lifetime(
        cx: &mut TestAppContext,
    ) {
        let (view, ui) = cx.add_window_view(|_, _| Fixture {
            show: true,
            covered: false,
            width: 120.,
            scroll: Default::default(),
        });
        ui.update(|w, _| w.activate_window());
        redraw(ui);
        ui.simulate_event(MouseMoveEvent {
            position: point(px(20.), px(20.)),
            pressed_button: None,
            modifiers: Default::default(),
        });
        redraw(ui);
        let (count, current) = snapshot(ui);
        assert_eq!(count, 1);
        assert_eq!(
            current.as_ref().unwrap().bounds.size,
            size(px(120.), px(60.))
        );
        assert!(current.unwrap().text.contains("한국어 · English"));
        // An exit/click must stay dismissed across redraws until another hover.
        // macOS's is_window_hovered alone reports activation, not pointer exit.
        ui.simulate_event(gpui::MouseExitEvent {
            position: point(px(20.), px(20.)),
            ..Default::default()
        });
        redraw(ui);
        assert!(snapshot(ui).1.is_none());
        ui.simulate_event(MouseMoveEvent {
            position: point(px(20.), px(20.)),
            pressed_button: None,
            modifiers: Default::default(),
        });
        redraw(ui);
        assert!(snapshot(ui).1.is_some());
        view.update(ui, |v, cx| {
            v.scroll.set_offset(point(px(0.), px(-110.)));
            cx.notify();
        });
        redraw(ui);
        assert_eq!(snapshot(ui).1.unwrap().text, "두 번째 항목");
        view.update(ui, |v, cx| {
            v.scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        });
        redraw(ui);
        view.update(ui, |v, cx| {
            v.width = 80.;
            cx.notify();
        });
        redraw(ui);
        assert_eq!(snapshot(ui).1.unwrap().bounds.size.width, px(80.));
        view.update(ui, |v, cx| {
            v.covered = true;
            cx.notify();
        });
        redraw(ui);
        assert!(
            snapshot(ui).1.is_none(),
            "covered targets cannot retain a native region"
        );
        view.update(ui, |v, cx| {
            v.covered = false;
            v.show = false;
            cx.notify();
        });
        redraw(ui);
        assert_eq!(snapshot(ui), (0, None));
        // Restoring the element starts from current geometry rather than a stale rectangle.
        view.update(ui, |v, cx| {
            v.show = true;
            cx.notify();
        });
        redraw(ui);
        assert_eq!(snapshot(ui).1.unwrap().bounds.size.width, px(80.));
        let weak = ui.update(|w, cx| {
            cx.global::<Registry>().0[&Window::window_handle(w).window_id()].clone()
        });
        ui.update(|w, _| w.remove_window());
        ui.run_until_parked();
        assert!(
            weak.upgrade().is_none(),
            "closing the window releases its native manager"
        );
    }
}
