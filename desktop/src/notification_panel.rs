use super::*;

const MARGIN: f32 = 8.;
const WIDTH: f32 = 360.;

impl Desktop {
    pub(super) fn render_notification_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewport = window.viewport_size();
        let panel = div()
            .id("notification-panel")
            .debug_selector(|| "notification-panel".into())
            .w(px(WIDTH.min((f32::from(viewport.width) - 2. * MARGIN).max(0.))))
            .max_h(px((f32::from(viewport.height) - 60.).max(0.)))
            .flex().flex_col().min_h_0()
            .p_3().gap_2()
            .bg(rgb(self.palette.panel))
            .border_1().border_color(rgb(self.palette.line)).rounded_lg().shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, w, cx| { w.prevent_default(); cx.stop_propagation(); })
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(div().flex().items_center().flex_shrink_0()
                .child(div().id("notification-title").text_lg().flex_1().child(t("알림"))
                    .native_tooltip(t("이번 실행 중 최대 100건. 읽음은 질문 응답·승인 처리를 뜻하지 않습니다.")))
                .child(self.button("close-notifications", t("닫기"), cx, |s, _, cx| s.dismiss_dialog(cx))))
            .child(div().id("notification-list").debug_selector(|| "notification-list".into())
                .min_h_0().overflow_y_scroll().track_scroll(&self.modal.scroll)
                .child(self.recent_agents(cx)));
        div()
            .id("notification-overlay")
            .debug_selector(|| "notification-overlay".into())
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| {
                    s.dismiss_dialog(cx);
                    w.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|s, _, w, cx| {
                    s.dismiss_dialog(cx);
                    w.prevent_default();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(AtNotificationButton {
                child: panel.into_any_element(),
                anchor: self.notification_anchor.clone(),
            })
    }
}

fn panel_origin(
    anchor: Bounds<Pixels>,
    panel: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    point(
        px(f32::from(anchor.right() - panel.width).clamp(
            MARGIN,
            (f32::from(viewport.width - panel.width) - MARGIN).max(MARGIN),
        )),
        px(f32::from(anchor.top() - panel.height) - MARGIN).max(px(MARGIN)),
    )
}

/// Resolve after the status button's prepaint, including the first resize frame.
struct AtNotificationButton {
    child: AnyElement,
    anchor: Rc<RefCell<Bounds<Pixels>>>,
}
impl IntoElement for AtNotificationButton {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for AtNotificationButton {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Default::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = window.layout_bounds(*child);
        let origin = panel_origin(*self.anchor.borrow(), bounds.size, window.viewport_size());
        window.with_element_offset(origin - bounds.origin, |w| {
            self.child.prepaint(w, cx);
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
