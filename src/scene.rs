use super::*;
pub(super) fn fit_text(p: &Painter, center: Pos2, text: &str, max: f32, size: f32, color: Color32) {
    let mut font = FontId::proportional(size);
    let mut g = p.layout_no_wrap(text.into(), font.clone(), color);
    if g.size().x > max {
        font.size = (font.size * max / g.size().x).max(9.0);
        g = p.layout_no_wrap(text.into(), font, color);
    }
    p.galley(center - g.size() * 0.5, g, color);
}
fn paint_image(p: &Painter, t: &TextureHandle, rect: Rect, cover: bool) {
    let sz = t.size_vec2();
    let aspect = sz.x / sz.y;
    let target = rect.width() / rect.height();
    let mut uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let mut dest = rect;
    if cover {
        if aspect > target {
            let w = target / aspect;
            uv.min.x = (1.0 - w) / 2.0;
            uv.max.x = 1.0 - uv.min.x;
        } else {
            let h = aspect / target;
            uv.min.y = (1.0 - h) / 2.0;
            uv.max.y = 1.0 - uv.min.y;
        }
    } else {
        let scale = (rect.width() / sz.x).min(rect.height() / sz.y);
        dest = Rect::from_center_size(rect.center(), sz * scale);
    }
    p.image(t.id(), dest, uv, Color32::WHITE);
}
// The stage's design, at 850 x 590 (everything is multiplied by the stage's scale).
const PAD: f32 = 32.0; // margin around the stage's content
const BRAND_LINE: f32 = 20.0; // the brand name's line at the top left
const ROW_GAP: f32 = 5.0; // between the brand line, the logo row and the titles
const BAND_MIN: f32 = 63.0; // under a brand name the top band keeps room for a logo row, so the titles stay put when a logo is added
const LOGO_BOX: Vec2 = vec2(95.0, 38.0); // one logo at 100 %
const EVENT_LINE: f32 = 36.0; // the event name, 27 px letters
const SUBTITLE_LINE: f32 = 18.0; // the subtitle, 12 px letters
const TITLE_GAP: f32 = 7.0; // between the two
const AREA_GAP: f32 = 12.8; // between the header and the timer's room (with every line shown the timer's middle is at 61 % of the height)
const FOOT: f32 = 44.0; // the status line at the bottom
const AREA_FULL: f32 = 372.2; // the timer's room while brand, event name and subtitle are all shown
const RING_RADIUS: f32 = 138.65; // the ring in that room
const BASE: [u8; 3] = [18, 27, 35]; // under everything, and around a background image that doesn't cover the stage
const GRADIENT: [[u8; 3]; 4] = [[24, 42, 47], [15, 25, 37], [17, 26, 35], [20, 34, 39]]; // corners from top left, clockwise, without a background image
/// The colour the stage shows behind its logos: the background image's mean colour over the base,
/// darkened by 어둡게; without an image, the mean of the gradient's corners.
pub(super) fn behind(s: &Settings, pics: &Pictures) -> [f32; 3] {
    let f = |c: [u8; 3]| c.map(|v| v as f32 / 255.0);
    match pics.mean.get(&s.background).filter(|_| pics.contains_key(&s.background)) {
        Some((mean, opaque)) => {
            let base = f(BASE);
            [0, 1, 2].map(|i| (mean[i] * opaque + base[i] * (1.0 - opaque)) * (1.0 - s.dim))
        }
        None => [0, 1, 2].map(|i| GRADIENT.iter().map(|c| f(*c)[i]).sum::<f32>() / 4.0),
    }
}
/// What the header has to show.
#[derive(Clone, Copy, Default)]
pub(super) struct Header {
    pub brand: bool,
    pub event: bool,
    pub subtitle: bool,
    /// The height of the logo row at the top (38 at 100 %), when logos sit there.
    pub top_logos: Option<f32>,
}
/// Where the parts of the stage go: the middle of each header line that has something in it
/// (the top of the logo row), the room that is left for the timer, and the timer's scale.
#[derive(Debug)]
pub(super) struct Stage {
    pub brand: Option<f32>,
    pub logos: Option<f32>,
    pub event: Option<f32>,
    pub subtitle: Option<f32>,
    pub area: Rect,
    pub timer: f32,
}
/// Lays the stage out the way VS Event Timer 1.1.0 did (its stage was a grid of rows `auto 1fr`,
/// an empty subtitle was hidden and an empty top band kept only a margin): the header takes
/// what its content needs and the timer gets all the rest, drawn to the size of that room. So
/// an event name or subtitle that is left empty gives its room to the timer. With brand,
/// event name and subtitle all there, the timer has the stage's own scale.
pub(super) fn layout(rect: Rect, scale: f32, h: Header) -> Stage {
    let (mut y, mut filled) = (rect.top() + PAD * scale, false);
    let mut stage = Stage { brand: None, logos: None, event: None, subtitle: None, area: rect, timer: scale };
    if h.brand {
        stage.brand = Some(y + BRAND_LINE * scale / 2.0);
        y += BRAND_LINE * scale;
        filled = true;
    }
    if let Some(logo) = h.top_logos {
        if filled {
            y += ROW_GAP * scale;
        }
        stage.logos = Some(y);
        y += logo * scale;
        filled = true;
    }
    if h.event || h.subtitle {
        if h.brand {
            y = y.max(rect.top() + (PAD + BAND_MIN) * scale);
        }
        if filled {
            y += ROW_GAP * scale;
        }
        if h.event {
            stage.event = Some(y + EVENT_LINE * scale / 2.0);
            y += EVENT_LINE * scale;
        }
        if h.subtitle {
            if h.event {
                y += TITLE_GAP * scale;
            }
            stage.subtitle = Some(y + SUBTITLE_LINE * scale / 2.0);
            y += SUBTITLE_LINE * scale;
        }
        filled = true;
    }
    if filled {
        y += AREA_GAP * scale;
    }
    let bottom = rect.bottom() - FOOT * scale;
    stage.area = Rect::from_min_max(pos2(rect.left(), y.min(bottom)), pos2(rect.right(), bottom));
    // As large as the room is high, but the ring never wider than 52 % of the stage; never tiny.
    stage.timer = (stage.area.height() / AREA_FULL).min(rect.width() * 0.26 / RING_RADIUS).max(scale * 0.25);
    stage
}
/// Paints the event stage and returns where the timer itself is (hover buttons go on it).
pub(super) fn render(p: &Painter, rect: Rect, s: &Settings, v: &timer::View, textures: &Pictures, qna: bool, running: bool) -> Rect {
    render_as(p, rect, s, s.mode, v, textures, (qna, running))
}
/// The same stage in another display, for an output screen that shows something else than the editor's preview.
/// (qna, running): whether the Q&A part is on, and whether the timer runs.
pub(super) fn render_as(
    p: &Painter,
    rect: Rect,
    s: &Settings,
    mode: Mode,
    v: &timer::View,
    textures: &Pictures,
    (qna, running): (bool, bool),
) -> Rect {
    let p = p.with_clip_rect(rect);
    let a = s.accent();
    let scale = (rect.width() / 850.0).min(rect.height() / 590.0).max(0.35);
    p.rect_filled(rect, 12, Color32::from_rgb(BASE[0], BASE[1], BASE[2]));
    if let Some(t) = textures.get(&s.background) {
        paint_image(&p, t, rect, s.cover);
        p.rect_filled(rect, 0, Color32::from_black_alpha((s.dim * 255.0) as u8));
    } else {
        let mut mesh = egui::epaint::Mesh::default();
        let colors = GRADIENT.map(|c| Color32::from_rgb(c[0], c[1], c[2]));
        for (pos, col) in [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()].into_iter().zip(colors) {
            mesh.colored_vertex(pos, col);
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(Shape::mesh(mesh));
        for i in 0..5 {
            let rr = rect.width() * 0.45 + i as f32 * 46.0 * scale;
            p.circle_stroke(
                pos2(rect.right() + rect.width() * 0.1, rect.top() - rect.height() * 0.15),
                rr,
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(132, 226, 190, 9)),
            );
        }
    }
    p.rect_stroke(rect, 12, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    let pad = PAD * scale;
    let logo_box = LOGO_BOX * scale * (s.logo_size / 100.0);
    let count = [&s.logo, &s.logo2].into_iter().filter(|key| textures.contains_key(key)).count();
    let (brand, event, subtitle) = (s.brand.trim(), s.event.trim(), s.subtitle.trim());
    let stage = layout(
        rect,
        scale,
        Header {
            brand: !brand.is_empty(),
            event: !event.is_empty(),
            subtitle: !subtitle.is_empty(),
            top_logos: (count > 0 && s.logo_position < 3).then(|| LOGO_BOX.y * s.logo_size / 100.0),
        },
    );
    if let Some(y) = stage.brand {
        p.text(pos2(rect.left() + pad, y), Align2::LEFT_CENTER, brand, FontId::proportional(15.0 * scale), a);
    }
    let width = count as f32 * logo_box.x + (count.saturating_sub(1)) as f32 * 12.0 * scale;
    let bg = behind(s, textures);
    for (idx, key) in [&s.logo, &s.logo2].into_iter().filter(|key| textures.contains_key(key)).enumerate() {
        if let Some((t, _)) = textures.logo(key, s.logo_tone, Some(bg)) {
            let left = match s.logo_position % 3 {
                0 => rect.left() + pad,
                1 => rect.center().x - width / 2.0,
                _ => rect.right() - pad - width,
            };
            let y = match stage.logos {
                Some(top) => top + logo_box.y * 0.5,
                None => rect.bottom() - pad - logo_box.y * 0.5 - 18.0 * scale,
            };
            let center = pos2(left + logo_box.x * 0.5 + idx as f32 * (logo_box.x + 12.0 * scale), y);
            paint_image(&p, t, Rect::from_center_size(center, logo_box), false);
        }
    }
    if let Some(y) = stage.event {
        fit_text(&p, pos2(rect.center().x, y), event, rect.width() - pad * 2.0, 27.0 * scale, WHITE);
    }
    if let Some(y) = stage.subtitle {
        fit_text(&p, pos2(rect.center().x, y), subtitle, rect.width() - pad * 2.0, 12.0 * scale, MUTED);
    }
    let color = phase_color(v.phase, a);
    // The timer sits in the middle of its room and is drawn at the room's scale `k`.
    let (center, k) = (stage.area.center(), stage.timer);
    let phase_text = match v.phase {
        Phase::Ready => t("시작할 준비가 되었습니다"),
        Phase::Warning => t("마무리를 준비해주세요"),
        Phase::End => t("시간이 종료되었습니다"),
        Phase::Over => t("설정 시간을 초과했습니다"),
        _ => {
            if running {
                t("진행 중")
            } else {
                t("일시정지")
            }
        }
    };
    let timer_label = if qna {
        "Q&A"
    } else if s.count_up {
        "ELAPSED TIME"
    } else {
        "TIME REMAINING"
    };
    let timer_rect = match mode {
        Mode::Circle => {
            let center = on_pixels(&p, center);
            let radius = RING_RADIUS * k;
            let width = 10.0 * k;
            ring(&p, center, radius, width, Color32::from_rgb(41, 58, 64));
            let steps = 180;
            let angle = std::f32::consts::TAU * v.fill;
            if v.fill >= 0.999 {
                ring(&p, center, radius, width, color);
            } else if angle > 0.0 {
                let pts = (0..=steps)
                    .map(|i| {
                        let t = -std::f32::consts::FRAC_PI_2 + angle * i as f32 / steps as f32;
                        center + vec2(t.cos(), t.sin()) * radius
                    })
                    .collect();
                p.add(Shape::line(pts, Stroke::new(width, color)));
            }
            for i in 0..60 {
                let t = std::f32::consts::TAU * i as f32 / 60.0 - std::f32::consts::FRAC_PI_2;
                let dir = vec2(t.cos(), t.sin());
                let len = if i % 5 == 0 { 6.0 } else { 3.0 } * k;
                p.line_segment(
                    [center + dir * (radius - 17.0 * k), center + dir * (radius - 17.0 * k - len)],
                    Stroke::new(1.0_f32, Color32::from_rgb(60, 78, 83)),
                );
            }
            fit_text(&p, center - vec2(0.0, 39.0 * k), timer_label, radius * 1.65, 10.0 * k, MUTED);
            fit_text(&p, center + vec2(0.0, 3.0 * k), &v.text, radius * 1.66, 68.0 * k, WHITE);
            fit_text(&p, center + vec2(0.0, 51.0 * k), if qna { t("질의응답") } else { t("발표 시간") }, radius * 1.6, 12.0 * k, color);
            Rect::from_center_size(center, Vec2::splat(radius * 2.0 + width))
        }
        Mode::Bar => {
            fit_text(&p, center - vec2(0.0, 77.0 * k), timer_label, rect.width() * 0.8, 11.0 * k, MUTED);
            fit_text(&p, center - vec2(0.0, 20.0 * k), &v.text, rect.width() * 0.78, 99.0 * k, WHITE);
            let bar = Rect::from_center_size(center + vec2(0.0, 64.0 * k), vec2(rect.width() * 0.76, 21.0 * k));
            p.rect_filled(bar, 9, Color32::from_rgb(41, 58, 64));
            if v.fill > 0.0 {
                let fill = Rect::from_min_size(bar.min, vec2(bar.width() * v.fill, bar.height()));
                p.rect_filled(fill, 9, color);
            }
            p.text(
                bar.left_bottom() + vec2(0.0, 18.0 * k),
                Align2::LEFT_TOP,
                if s.count_up {
                    tf!("경과 {0}%", format!("{:.0}", v.fill * 100.0))
                } else {
                    tf!("남은 시간 {0}%", format!("{:.0}", v.remaining * 100.0))
                },
                FontId::proportional(11.0 * k),
                MUTED,
            );
            p.text(
                bar.right_bottom() + vec2(0.0, 18.0 * k),
                Align2::RIGHT_TOP,
                if qna { "Q&A" } else { "PRESENTATION" },
                FontId::proportional(11.0 * k),
                MUTED,
            );
            Rect::from_min_max(pos2(bar.left(), center.y - 85.0 * k), pos2(bar.right(), bar.bottom() + 32.0 * k))
        }
        Mode::Number => {
            fit_text(&p, center - vec2(0.0, 86.0 * k), timer_label, rect.width() * 0.8, 11.0 * k, MUTED);
            fit_text(&p, center, &v.text, rect.width() * 0.83, 134.0 * k, color);
            fit_text(&p, center + vec2(0.0, 88.0 * k), if qna { t("질의응답") } else { t("발표 시간") }, rect.width() * 0.7, 14.0 * k, MUTED);
            Rect::from_center_size(center + vec2(0.0, 6.0 * k), vec2(rect.width() * 0.6, 200.0 * k))
        }
    };
    fit_text(&p, pos2(rect.center().x, rect.bottom() - 28.0 * scale), phase_text, rect.width() * 0.8, 12.0 * scale, color);
    timer_rect.intersect(rect)
}
