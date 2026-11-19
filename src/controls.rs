//! Buttons that appear over a timer while the mouse is on it: 리셋, 재시작 (시작 before the
//! first start) and 멈춤, plus 편집 화면 and 닫기 on the floating timer.
//!
//! The bar is pill-shaped with 40 px buttons (28 px on the floating timer) and fades in and
//! out, as in VS Event Timer 1.1.0, this program's Electron predecessor; its line icons are
//! drawn again here on a 24-unit grid.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cmd {
    Reset,
    Play,
    Pause,
    Editor,
    Quit,
}

/// What the timer is doing, which decides what each button offers.
#[derive(Clone, Copy)]
pub(super) struct State {
    pub running: bool,
    pub started: bool,
    pub accent: Color32,
}

/// One bar's size: the stage (preview, full screen, audience window) or the floating timer.
#[derive(Clone, Copy)]
pub(super) struct Style {
    pub button: f32,
    pub icon: f32,
    pub label: f32,
    pub utility: bool,
}
pub(super) const STAGE: Style = Style { button: CONTROL_BUTTON, icon: 20.0, label: 14.0, utility: false };
pub(super) const FLOATING: Style = Style { button: CONTROL_BUTTON_FLOATING, icon: 16.0, label: 12.0, utility: true };

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Icon {
    Reset,
    Play,
    Pause,
    Gear,
    Close,
}

#[derive(Debug, PartialEq)]
pub(super) struct Item {
    pub cmd: Cmd,
    pub icon: Icon,
    pub label: &'static str,
    pub tip: &'static str,
    pub enabled: bool,
    /// The likely next press, filled with the brand colour.
    pub primary: bool,
}

pub(super) fn items(st: State, utility: bool) -> Vec<Item> {
    let mut v = vec![
        Item {
            cmd: Cmd::Reset, icon: Icon::Reset, label: t("리셋"), tip: t("처음 시간으로 되돌리기 (R)"), enabled: st.started, primary: false
        },
        Item {
            cmd: Cmd::Play,
            icon: Icon::Play,
            label: if st.started { t("재시작") } else { t("시작") },
            tip: t("시작 / 이어서 진행 (Space)"),
            enabled: !st.running,
            primary: !st.running,
        },
        Item { cmd: Cmd::Pause, icon: Icon::Pause, label: t("멈춤"), tip: t("잠시 멈춤 (Space)"), enabled: st.running, primary: st.running },
    ];
    if utility {
        v.push(Item { cmd: Cmd::Editor, icon: Icon::Gear, label: "", tip: t("편집 화면으로"), enabled: true, primary: false });
        v.push(Item { cmd: Cmd::Quit, icon: Icon::Close, label: "", tip: t("닫기 (앱 종료)"), enabled: true, primary: false });
    }
    v
}

/// A drawn bar: where it is, where each button can be pressed, and this frame's press.
pub(super) struct Bar {
    pub rect: Rect,
    pub buttons: Vec<(Cmd, Rect)>,
    pub pressed: Option<Cmd>,
}

/// Draws the bar with its bottom centre at `anchor`, kept inside `bounds`, at `opacity`
/// (0 hides it and it can't be pressed).
pub(super) fn bar(ui: &Ui, salt: &str, bounds: Rect, anchor: Pos2, style: Style, st: State, opacity: f32) -> Bar {
    const PAD: f32 = 4.0;
    const GAP: f32 = 2.0;
    const SEP: f32 = 9.0;
    let items = items(st, style.utility);
    let p = ui.painter();
    let text_w = |s: &str| p.layout_no_wrap(s.to_owned(), FontId::proportional(style.label), WHITE).size().x;
    let widths = |labels: bool| -> Vec<f32> {
        items
            .iter()
            .map(|it| if labels && !it.label.is_empty() { style.button * 0.7 + style.icon + 6.0 + text_w(it.label) } else { style.button })
            .collect()
    };
    let total = |w: &[f32]| PAD * 2.0 + w.iter().sum::<f32>() + GAP * (w.len() as f32 - 1.0) + if style.utility { SEP } else { 0.0 };
    let room = (bounds.width() - 8.0).max(40.0);
    let mut labels = true;
    let mut w = widths(true);
    if total(&w) > room {
        labels = false;
        w = widths(false);
    }
    let k = (room / total(&w)).clamp(0.55, 1.0);
    let size = vec2(total(&w) * k, (style.button + PAD * 2.0) * k);
    let mut rect = Rect::from_min_size(pos2(anchor.x - size.x / 2.0, anchor.y - size.y), size);
    rect = rect.translate(vec2(
        (bounds.left() + 4.0 - rect.left()).max(0.0) + (bounds.right() - 4.0 - rect.right()).min(0.0),
        (bounds.top() + 4.0 - rect.top()).max(0.0) + (bounds.bottom() - 4.0 - rect.bottom()).min(0.0),
    ));
    if opacity <= 0.01 {
        return Bar { rect, buttons: Vec::new(), pressed: None };
    }
    let o = opacity;
    let radius = size.y / 2.0;
    p.rect_filled(rect.expand(2.0).translate(vec2(0.0, 1.0)), radius + 2.0, Color32::from_black_alpha((70.0 * o) as u8));
    p.rect_filled(rect, radius, Color32::from_rgba_unmultiplied(23, 28, 36, (240.0 * o) as u8));
    p.rect_stroke(rect, radius, Stroke::new(1.0_f32, LINE.gamma_multiply(o)), StrokeKind::Inside);
    let mut pressed = None;
    let mut buttons = Vec::new();
    let mut x = rect.left() + PAD * k;
    for (i, it) in items.iter().enumerate() {
        if style.utility && it.cmd == Cmd::Editor {
            let sx = x + SEP * k / 2.0 - GAP * k / 2.0;
            p.line_segment([pos2(sx, rect.top() + 9.0 * k), pos2(sx, rect.bottom() - 9.0 * k)], Stroke::new(1.0_f32, LINE.gamma_multiply(o)));
            x += SEP * k;
        }
        let r = Rect::from_min_size(pos2(x, rect.top() + PAD * k), vec2(w[i] * k, style.button * k));
        x += w[i] * k + GAP * k;
        // The press area takes in the bar's padding and half the gaps, so small buttons are easy to hit.
        let hit = r.expand2(vec2(GAP * k / 2.0, PAD * k));
        buttons.push((it.cmd, hit));
        let sense = if it.enabled { Sense::click() } else { Sense::hover() };
        let mut resp = ui.interact(hit, ui.id().with((salt, i)), sense);
        if !style.utility {
            resp = resp.on_hover_text(it.tip);
        }
        let ink = if !it.enabled {
            MUTED.gamma_multiply(0.45)
        } else if it.primary {
            BG
        } else {
            WHITE
        };
        if it.enabled && it.primary {
            let fill = if resp.hovered() { st.accent.gamma_multiply(0.85) } else { st.accent };
            p.rect_filled(r, 8.0 * k, fill.gamma_multiply(o));
        } else if it.enabled && resp.hovered() {
            p.rect_filled(r, 8.0 * k, Color32::from_white_alpha((26.0 * o) as u8));
        }
        if it.enabled && resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        let ink = ink.gamma_multiply(o);
        if labels && !it.label.is_empty() {
            let g = p.layout_no_wrap(it.label.to_owned(), FontId::proportional(style.label * k), ink);
            let content = style.icon * k + 6.0 * k + g.size().x;
            let left = r.center().x - content / 2.0;
            icon(p, it.icon, pos2(left + style.icon * k / 2.0, r.center().y), style.icon * k, ink);
            p.galley(pos2(left + style.icon * k + 6.0 * k, r.center().y - g.size().y / 2.0), g, ink);
        } else {
            icon(p, it.icon, r.center(), style.icon * k, ink);
        }
        if resp.clicked() && it.enabled && o > 0.5 {
            pressed = Some(it.cmd);
        }
    }
    Bar { rect, buttons, pressed }
}

/// Line icons on a 24-unit grid, as in 1.1.0.
pub(super) fn icon(p: &Painter, icon: Icon, c: Pos2, size: f32, color: Color32) {
    let u = size / 24.0;
    let at = |x: f32, y: f32| c + vec2((x - 12.0) * u, (y - 12.0) * u);
    let stroke = Stroke::new((2.0 * u).max(1.2), color);
    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![at(8.0, 5.0), at(19.0, 12.0), at(8.0, 19.0)], color, Stroke::NONE));
        }
        Icon::Pause => {
            p.rect_filled(Rect::from_min_max(at(7.0, 5.0), at(11.0, 19.0)), u, color);
            p.rect_filled(Rect::from_min_max(at(13.0, 5.0), at(17.0, 19.0)), u, color);
        }
        Icon::Reset => {
            // M5 12a7 7 0 1 0 2.05-4.95 / M5 4v4h4: the long way round from the left, arrow top-left.
            let n = 32;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let t = std::f32::consts::PI - 1.75 * std::f32::consts::PI * i as f32 / n as f32;
                    at(12.0 + 7.0 * t.cos(), 12.0 + 7.0 * t.sin())
                })
                .collect();
            p.add(Shape::line(pts, stroke));
            p.add(Shape::line(vec![at(5.0, 4.0), at(5.0, 8.0), at(9.0, 8.0)], stroke));
        }
        Icon::Gear => {
            for t in 0..8 {
                let a = t as f32 * std::f32::consts::TAU / 8.0;
                let pts: Vec<Pos2> = (0..=4)
                    .map(|i| {
                        let b = a - 0.2 + 0.1 * i as f32;
                        at(12.0 + 7.5 * b.cos(), 12.0 + 7.5 * b.sin())
                    })
                    .collect();
                p.add(Shape::line(pts, Stroke::new(3.0 * u, color)));
            }
            p.circle_stroke(c, 4.0 * u, stroke);
        }
        Icon::Close => {
            p.line_segment([at(6.0, 6.0), at(18.0, 18.0)], stroke);
            p.line_segment([at(18.0, 6.0), at(6.0, 18.0)], stroke);
        }
    }
}

/// The corner handle for resizing: 24 px, two diagonal strokes (as in 1.1.0).
pub(super) fn grip(p: &Painter, r: Rect, opacity: f32) {
    let u = (r.width() - 8.0) / 16.0;
    let at = |x: f32, y: f32| r.min + vec2(4.0 + x * u, 4.0 + y * u);
    for (a, b) in [((14.0, 4.0), (4.0, 14.0)), ((14.0, 9.0), (9.0, 14.0))] {
        let seg = [at(a.0, a.1), at(b.0, b.1)];
        p.line_segment(seg, Stroke::new(3.0 * u, BG.gamma_multiply(0.7 * opacity)));
        p.line_segment(seg, Stroke::new(1.5 * u, WHITE.gamma_multiply(opacity)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buttons_follow_the_timer() {
        let ready = items(State { running: false, started: false, accent: WHITE }, false);
        assert_eq!(ready.iter().map(|i| i.label).collect::<Vec<_>>(), ["리셋", "시작", "멈춤"]);
        assert!(!ready[0].enabled && ready[1].enabled && ready[1].primary && !ready[2].enabled);
        let running = items(State { running: true, started: true, accent: WHITE }, true);
        assert_eq!(running[1].label, "재시작");
        assert!(running[0].enabled && !running[1].enabled && running[2].enabled && running[2].primary);
        assert_eq!(running.iter().map(|i| i.cmd).collect::<Vec<_>>(), [Cmd::Reset, Cmd::Play, Cmd::Pause, Cmd::Editor, Cmd::Quit]);
        let paused = items(State { running: false, started: true, accent: WHITE }, false);
        assert!(paused[1].enabled && paused[1].primary && !paused[2].enabled && paused[0].enabled);
    }
}
