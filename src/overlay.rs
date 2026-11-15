//! The floating timer: the ring, the bar or the digits (and, if asked for, the brand, event
//! name and logos) on a transparent window, with a dark halo so they read over any slide.
//! Its background is always see-through; the stage's background never comes along.
//!
//! Proportions follow the overlay of VS Event Timer 1.1.0, this program's Electron
//! predecessor (its style.css `body.overlay`, `body[data-mode=...]`); colours follow this
//! app's stage.
use super::*;

const HALO: Color32 = Color32::from_rgb(15, 19, 25);
/// rgba(255,255,255,0.18), style.css `--track` for the dark themes.
const TRACK: Color32 = Color32::from_rgba_premultiplied(46, 46, 46, 46);

/// The small line under the digits (app.js `frame`: status).
pub(super) fn status_text(v: &timer::View, count_up: bool, running: bool, started: bool, qna: bool, duration: f64) -> String {
    match v.phase {
        Phase::Warning => t("곧 종료").into(),
        Phase::End => t("시간 종료").into(),
        Phase::Over => t("시간 초과").into(),
        _ if !running && started => t("일시정지").into(),
        _ if qna => "Q&A".into(),
        _ if count_up => tf!("목표 {0}", timer::format_seconds(duration as u64)),
        _ => String::new(),
    }
}

/// Text with a halo (style.css `-webkit-text-stroke: 0.07em` + `text-shadow 0.08em`),
/// digits laid out in equal cells like `font-variant-numeric: tabular-nums`, so the time
/// doesn't shake as it counts. Shrinks to fit `max_w`.
pub(super) fn halo_text(p: &Painter, center: Pos2, text: &str, size: f32, max_w: f32, color: Color32) -> Rect {
    if text.is_empty() || size <= 0.0 {
        return Rect::from_center_size(center, Vec2::ZERO);
    }
    let lay = |size: f32| {
        let font = FontId::new(size, bold());
        let zero = p.layout_no_wrap("0".into(), font.clone(), Color32::WHITE).size().x;
        let glyphs: Vec<_> = text
            .chars()
            .map(|ch| {
                let g = p.layout_no_wrap(ch.to_string(), font.clone(), Color32::WHITE);
                let w = if ch.is_ascii_digit() { zero } else { g.size().x };
                (g, w)
            })
            .collect();
        let spacing = -0.02 * size;
        let width = glyphs.iter().map(|g| g.1).sum::<f32>() + spacing * glyphs.len().saturating_sub(1) as f32;
        (glyphs, width, spacing)
    };
    let mut font_size = size;
    let (mut glyphs, mut width, mut spacing) = lay(font_size);
    if width > max_w && max_w > 0.0 {
        font_size *= max_w / width;
        (glyphs, width, spacing) = lay(font_size);
    }
    let height = glyphs.iter().map(|g| g.0.size().y).fold(0.0, f32::max);
    // About half of the 0.07em centred stroke shows outside the glyph; the shadow stays soft.
    let stroke = font_size * 0.03;
    let shadow = font_size * 0.05;
    let around = [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0)];
    let top = center.y - height / 2.0;
    let mut x = center.x - width / 2.0;
    for (g, w) in &glyphs {
        let pos = pos2(x + (w - g.size().x) / 2.0, top);
        for (dx, dy) in around {
            p.galley_with_override_text_color(pos + vec2(dx, dy) * shadow, g.clone(), HALO.gamma_multiply(0.22));
        }
        for (dx, dy) in around {
            p.galley_with_override_text_color(pos + vec2(dx, dy) * stroke, g.clone(), HALO);
        }
        p.galley_with_override_text_color(pos, g.clone(), color);
        x += w + spacing;
    }
    Rect::from_center_size(center, vec2(width, height))
}

fn digits_color(phase: Phase) -> Color32 {
    match phase {
        Phase::Warning => WARN,
        Phase::End | Phase::Over => END,
        _ => WHITE,
    }
}

/// Where the floating timer has paint. On a layered window with per-pixel alpha
/// (UpdateLayeredWindow, ULW_ALPHA) Windows sends the mouse on fully transparent pixels to
/// the window behind; the floating timer behaves the same way, so outside these areas it
/// lets clicks through to the slides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Solid {
    Rect(Rect),
    Disc(Pos2, f32),
}

impl Solid {
    #[cfg(test)]
    pub(super) fn contains(&self, p: Pos2) -> bool {
        match *self {
            Solid::Rect(r) => r.contains(p),
            Solid::Disc(c, r) => c.distance(p) <= r,
        }
    }

    /// In window pixels, a little larger so the halo is never cut off by the window region.
    pub(super) fn to_px(&self, ppp: f32) -> win::HitShape {
        let px = |r: Rect| {
            let r = r.expand(3.0);
            let (x, y) = ((r.min.x * ppp).floor() as i32, (r.min.y * ppp).floor() as i32);
            win::PxRect { x, y, w: (r.max.x * ppp).ceil() as i32 - x, h: (r.max.y * ppp).ceil() as i32 - y }
        };
        match *self {
            Solid::Rect(r) => win::HitShape::Rect(px(r)),
            Solid::Disc(c, r) => win::HitShape::Ellipse(px(Rect::from_center_size(c, Vec2::splat(r * 2.0)))),
        }
    }
}

/// A filled disc as a polygon. `circle_filled` draws small discs from a texture, and its soft
/// edge comes out too bright on top of the same colour (a pale ring around each line end).
fn disc(p: &Painter, c: Pos2, r: f32, color: Color32) {
    let n = 28;
    let pts = (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// The share of the window's height that brand, event name and subtitle take together.
const TITLES_SHARE: f32 = 0.30;
/// Titles and logos never take more than this share: the timer is the point.
const TITLES_MOST: f32 = 0.5;

/// Brand, event name, subtitle and logos for the floating timer, drawn straight onto the
/// see-through window. Returns the room left for the timer.
fn titles(p: &Painter, rect: Rect, s: &Settings, textures: &HashMap<String, TextureHandle>, solid: &mut Vec<Solid>) -> Rect {
    // Font sizes, line height, gaps and logo height at scale 1.
    const LINE: f32 = 1.3;
    const GAP: f32 = 2.0;
    const PAD: f32 = 5.0;
    const LOGO: f32 = 22.0;
    let sizes = [11.5, 17.0, 10.5];
    let lines: Vec<(&str, f32, Color32)> = [(s.brand.trim(), sizes[0], s.accent()), (s.event.trim(), sizes[1], WHITE), (s.subtitle.trim(), sizes[2], MUTED)]
        .into_iter()
        .filter(|line| !line.0.is_empty())
        .collect();
    let logos: Vec<&TextureHandle> = [&s.logo, &s.logo2].into_iter().filter_map(|key| textures.get(key)).collect();
    if lines.is_empty() && logos.is_empty() {
        return rect;
    }
    let text_h = |k: f32| lines.iter().map(|line| line.1 * LINE * k).sum::<f32>() + GAP * k * lines.len().saturating_sub(1) as f32;
    let logo_h = |k: f32| if logos.is_empty() { 0.0 } else { LOGO * k * s.logo_size / 100.0 + GAP * 2.0 * k };
    let total = |k: f32| text_h(k) + logo_h(k) + PAD * 2.0 * k;
    // Three lines take TITLES_SHARE of the height; everything scales with the window.
    let mut k = rect.height() * TITLES_SHARE / (sizes.iter().sum::<f32>() * LINE + GAP * 2.0 + PAD * 2.0);
    let most = rect.height() * TITLES_MOST;
    if total(k) > most {
        k *= most / total(k);
    }
    let logos_on_top = s.logo_position < 3;
    // Draws the logos side by side from `top` down, on the side the person chose.
    let logo_row = |top: f32| -> Vec<Solid> {
        let h = LOGO * k * s.logo_size / 100.0;
        let gap = 6.0 * k;
        let boxes: Vec<Vec2> = logos
            .iter()
            .map(|t| {
                let size = t.size_vec2();
                let w = (h * size.x / size.y.max(1.0)).min(rect.width() * 0.45);
                vec2(w, w * size.y / size.x.max(1.0))
            })
            .collect();
        let row = boxes.iter().map(|b| b.x).sum::<f32>() + gap * boxes.len().saturating_sub(1) as f32;
        let mut x = match s.logo_position % 3 {
            0 => rect.left() + PAD * k,
            1 => rect.center().x - row / 2.0,
            _ => rect.right() - PAD * k - row,
        };
        let mut drawn = Vec::new();
        for (t, b) in logos.iter().zip(&boxes) {
            let at = Rect::from_min_size(pos2(x, top + (h - b.y) / 2.0), *b);
            p.image(t.id(), at, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            drawn.push(Solid::Rect(at));
            x += b.x + gap;
        }
        drawn
    };
    let mut y = rect.top() + PAD * k;
    if !logos.is_empty() && logos_on_top {
        solid.extend(logo_row(y));
        y += logo_h(k);
    }
    for (text, size, color) in &lines {
        let line = size * LINE * k;
        let drawn = halo_text(p, pos2(rect.center().x, y + line / 2.0), text, size * k, rect.width() * 0.94, *color);
        solid.push(Solid::Rect(drawn.expand(3.0)));
        y += line + GAP * k;
    }
    let top = if lines.is_empty() { y } else { y - GAP * k } + PAD * k;
    let mut bottom = rect.bottom();
    if !logos.is_empty() && !logos_on_top {
        bottom -= logo_h(k) + PAD * k;
        solid.extend(logo_row(bottom + GAP * 2.0 * k));
    }
    Rect::from_min_max(pos2(rect.left(), top.min(bottom)), pos2(rect.right(), bottom))
}

/// Paints the floating timer into the whole window `rect` and returns where it painted.
/// Nothing is behind it: the window stays see-through unless the person asked for a
/// backdrop (style.css `--backdrop`). With `overlay_scene` the brand, event name, subtitle
/// and logos come along above the timer, still on nothing.
pub(super) fn paint(p: &Painter, rect: Rect, s: &Settings, v: &timer::View, status: &str, textures: &HashMap<String, TextureHandle>) -> Vec<Solid> {
    let mut solid = Vec::new();
    if s.overlay_backdrop > 0.0 {
        p.rect_filled(rect, 16, HALO.gamma_multiply(s.overlay_backdrop));
        solid.push(Solid::Rect(rect));
    }
    let room = if s.overlay_scene { titles(p, rect, s, textures, &mut solid) } else { rect };
    timer(p, room, s, v, status, &mut solid);
    solid
}

/// The ring, bar or digits in `rect`.
fn timer(p: &Painter, rect: Rect, s: &Settings, v: &timer::View, status: &str, solid: &mut Vec<Solid>) {
    // style.css body.overlay #stage { padding: 3vh 3vw }
    let c = rect.shrink2(rect.size() * 0.03);
    let color = phase_color(v.phase, s.accent());
    let digits = digits_color(v.phase);
    // style.css .digits.long { transform: scale(0.72) }
    let long = if v.text.chars().count() > 6 { 0.72 } else { 1.0 };
    match s.mode {
        Mode::Circle => {
            // .viz-circle: min(94cqh, 84cqw), r 92 and stroke 9 on a 200 box.
            let d = (c.height() * 0.94).min(c.width() * 0.84);
            let center = c.center();
            let r = d * 92.0 / 200.0;
            let sw = d * 9.0 / 200.0;
            ring(p, center, r, sw + 2.0, HALO.gamma_multiply(0.6));
            ring(p, center, r, sw, TRACK);
            if v.fill >= 0.999 {
                // A full ring has no ends.
                ring(p, center, r, sw, color);
            } else if v.fill > 0.001 {
                let angle = std::f32::consts::TAU * v.fill;
                let steps = 120;
                let at = |t: f32| {
                    let a = -std::f32::consts::FRAC_PI_2 + angle * t;
                    center + vec2(a.cos(), a.sin()) * r
                };
                let pts: Vec<Pos2> = (0..=steps).map(|i| at(i as f32 / steps as f32)).collect();
                p.add(Shape::line(pts, Stroke::new(sw, color)));
                // stroke-linecap: round
                disc(p, at(0.0), sw / 2.0, color);
                disc(p, at(1.0), sw / 2.0, color);
            }
            let size = (c.height() * 0.23).min(c.width() * 0.20) * long;
            let status_size = (c.height() * 0.06).max(6.0);
            let block = if status.is_empty() { 0.0 } else { status_size * 1.55 };
            let y = center.y - block / 2.0;
            halo_text(p, pos2(center.x, y), &v.text, size, r * 1.6, digits);
            if !status.is_empty() {
                halo_text(p, pos2(center.x, y + size * 0.62 + status_size * 0.9), status, status_size, r * 1.5, MUTED);
            }
            solid.push(Solid::Disc(center, r + sw / 2.0 + 2.0));
        }
        Mode::Bar => {
            // body[data-mode='bar']: digits min(24cqw, 50cqh), bar 84cqw x clamp(4px, 6cqh, 44px), gap 4cqh.
            let size = (c.width() * 0.24).min(c.height() * 0.50) * long;
            let bar_h = (c.height() * 0.06).clamp(4.0, 44.0);
            let gap = c.height() * 0.04;
            let status_size = (c.height() * 0.08).max(7.0);
            let block = if status.is_empty() { 0.0 } else { status_size * 1.55 };
            let total = size * 1.1 + block + gap + bar_h;
            let top = c.center().y - total / 2.0;
            let text = halo_text(p, pos2(c.center().x, top + size * 0.55), &v.text, size, c.width() * 0.84, digits);
            if !status.is_empty() {
                halo_text(p, pos2(c.center().x, top + size * 1.1 + block / 2.0), status, status_size, c.width() * 0.84, MUTED);
            }
            let bar = Rect::from_min_size(pos2(c.center().x - c.width() * 0.42, top + size * 1.1 + block + gap), vec2(c.width() * 0.84, bar_h));
            solid.push(Solid::Rect(text.union(bar).expand(4.0)));
            p.rect_filled(bar.expand(1.0), bar_h / 2.0 + 1.0, HALO.gamma_multiply(0.6));
            p.rect_filled(bar, bar_h / 2.0, TRACK);
            if v.fill > 0.001 {
                let fill = Rect::from_min_size(bar.min, vec2(bar.width() * v.fill, bar_h));
                p.rect_filled(fill, (bar_h / 2.0).min(fill.width() / 2.0), color);
            }
        }
        Mode::Number => {
            // body[data-mode='time'] .digits { font-size: min(30cqw, 66cqh) }
            let size = (c.width() * 0.30).min(c.height() * 0.66) * long;
            let status_size = (c.height() * 0.08).max(7.0);
            let block = if status.is_empty() { 0.0 } else { status_size * 1.55 };
            let y = c.center().y - block / 2.0;
            let mut text = halo_text(p, pos2(c.center().x, y), &v.text, size, c.width() * 0.96, digits);
            if !status.is_empty() {
                text = text.union(halo_text(p, pos2(c.center().x, y + size * 0.62 + status_size * 0.9), status, status_size, c.width() * 0.9, MUTED));
            }
            solid.push(Solid::Rect(text.expand(4.0)));
        }
    }
}
