//! Output screens: as many extra windows as the event needs (the projector, the stage monitor, the
//! staff's laptop), each showing the timer in a form of its own: the stage's ring, bar or digits,
//! a detailed view for the presenter and staff, or a small one in the style of the floating timer.
//!
//! All screens show the one clock of the program (studio.rs `Studio::clock`), so there is nothing
//! to send between windows: a press of Start, Pause, Reset, a time adjustment or a new preset, from
//! the editor, from any screen, from a shortcut or from a system-wide hotkey, changes that clock
//! and every window is drawn from it in the same frame. A screen that is opened later reads the
//! same clock and so starts out showing the right time.
use super::*;

/// How many output screens can be open at once.
pub(super) const MAX_SCREENS: usize = 6;

/// What a screen shows. `Follow` is the editor's own display (the 1 / 2 / 3 keys and the buttons
/// above the preview change it), as the audience window always did.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub(super) enum Format {
    #[default]
    Follow,
    Circle,
    Bar,
    Number,
    Presenter,
    Mini,
}

/// What a format comes down to once `Follow` is settled.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Look {
    /// The event stage (brand, event name, logos, background) with a ring, a bar or digits.
    Stage(Mode),
    /// Time, progress and the figures a presenter or the staff want, on a plain background.
    Presenter,
    /// Digits and a thin bar on a plain background, as small as the window is made.
    Mini,
}

impl Format {
    pub const ALL: [Format; 6] = [Format::Follow, Format::Circle, Format::Bar, Format::Number, Format::Presenter, Format::Mini];

    pub fn label(self) -> &'static str {
        match self {
            Format::Follow => t("메인과 같게"),
            Format::Circle => t("원그래프"),
            Format::Bar => t("막대그래프"),
            Format::Number => t("123  숫자"),
            Format::Presenter => t("발표자 · 스태프 상세"),
            Format::Mini => t("미니 · 오버레이형"),
        }
    }

    /// `main`: the editor's display, which `Follow` takes.
    pub fn look(self, main: Mode) -> Look {
        match self {
            Format::Follow => Look::Stage(main),
            Format::Circle => Look::Stage(Mode::Circle),
            Format::Bar => Look::Stage(Mode::Bar),
            Format::Number => Look::Stage(Mode::Number),
            Format::Presenter => Look::Presenter,
            Format::Mini => Look::Mini,
        }
    }

    /// Whether the picture has a ring or bar that moves between the seconds.
    pub fn moves(self, main: Mode) -> bool {
        !matches!(self.look(main), Look::Stage(Mode::Number))
    }
}

/// One open output screen.
pub(super) struct Screen {
    pub id: u32,
    pub format: Format,
    /// The window is in full screen (on the monitor it stands on).
    pub full: bool,
    /// When the mouse last moved over it: the hover buttons and the cursor go away after a while.
    pub activity: Instant,
    /// Asked to close; the window goes at the end of the frame.
    pub closed: bool,
}

impl Screen {
    pub fn new(id: u32, format: Format) -> Self {
        Self { id, format, full: false, activity: Instant::now(), closed: false }
    }
}

/// The window of a screen.
pub(super) fn viewport_id(id: u32) -> ViewportId {
    ViewportId::from_hash_of(("screen", id))
}

/// Where a new screen's window opens: each a little down and to the right of the one before,
/// so that they do not hide each other.
pub(super) fn cascade(id: u32) -> Pos2 {
    let n = (id % 8) as f32;
    pos2(70.0 + 38.0 * n, 70.0 + 38.0 * n)
}

/// What the screens need to know besides the settings and the time on the clock.
pub(super) struct Facts {
    pub qna: bool,
    pub running: bool,
    pub started: bool,
    /// The clock's length, in seconds.
    pub duration: f64,
}

/// Paints one screen into `rect` and returns where the timer is (the hover buttons sit under it).
pub(super) fn paint(p: &Painter, rect: Rect, s: &Settings, look: Look, v: &timer::View, f: &Facts, pics: &Pictures) -> Rect {
    match look {
        Look::Stage(mode) => scene::render_as(p, rect, s, mode, v, pics, (f.qna, f.running)),
        Look::Presenter => presenter(p, rect, s, v, f),
        Look::Mini => mini(p, rect, s, v, f),
    }
}

/// Text that is made smaller until it fits `max`, placed by `anchor` at `at`.
fn fit_at(p: &Painter, anchor: Align2, at: Pos2, text: &str, max: f32, size: f32, color: Color32) -> Rect {
    let mut font = FontId::proportional(size);
    let mut g = p.layout_no_wrap(text.into(), font.clone(), color);
    if g.size().x > max && max > 0.0 {
        font.size = (font.size * max / g.size().x).max(8.0);
        g = p.layout_no_wrap(text.into(), font, color);
    }
    let r = anchor.anchor_size(at, g.size());
    p.galley(r.min, g, color);
    r
}

/// The word in the corner chip: where the timer stands.
pub(super) fn chip_text(v: &timer::View, f: &Facts) -> &'static str {
    match v.phase {
        Phase::Warning => t("곧 종료"),
        Phase::End => t("시간 종료"),
        Phase::Over => t("시간 초과"),
        _ if f.qna => t("질의응답"),
        _ if f.running => t("진행 중"),
        _ if f.started => t("일시정지"),
        _ => t("준비"),
    }
}

/// The four figures under the progress bar: (what it is, the value, whether to paint it as a warning).
pub(super) fn figures(s: &Settings, v: &timer::View, f: &Facts) -> [(&'static str, String, bool); 4] {
    let remaining = f.duration - v.elapsed;
    let left = if remaining > 0.0 {
        (t("남은 시간"), timer::format_seconds(remaining.ceil() as u64), false)
    } else if v.phase == Phase::Over {
        (t("초과 시간"), format!("+{}", timer::format_seconds((-remaining).floor() as u64)), true)
    } else {
        (t("남은 시간"), timer::format_seconds(0), true)
    };
    let next = if s.qna_enabled && !f.qna {
        (t("Q&A 예정"), timer::format_seconds(s.qna_duration as u64), false)
    } else if s.warning > 0.0 {
        (t("주의 알림"), timer::format_seconds(s.warning as u64), false)
    } else {
        (t("주의 알림"), "—".to_string(), false)
    };
    [
        (t("설정 시간"), timer::format_seconds(f.duration as u64), false),
        (t("경과 시간"), timer::format_seconds(v.elapsed.floor() as u64), false),
        left,
        next,
    ]
}

/// The presenter's and staff's view: brand and event name, where the timer stands, the time in
/// large digits, the progress, and the figures that matter (set time, elapsed, left or over, next).
fn presenter(p: &Painter, rect: Rect, s: &Settings, v: &timer::View, f: &Facts) -> Rect {
    // Too small for the figures and the bar: the compact view is what still reads.
    if rect.width() < 360.0 || rect.height() < 240.0 {
        return mini(p, rect, s, v, f);
    }
    let p = p.with_clip_rect(rect);
    let k = (rect.width() / 960.0).min(rect.height() / 540.0).max(0.35);
    let accent = s.accent();
    let color = phase_color(v.phase, accent);
    p.rect_filled(rect, 12, BG);
    p.rect_stroke(rect, 12, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    let pad = 28.0 * k;
    let inner = rect.width() - pad * 2.0;
    // The corner chip first: its width is what the titles must leave free.
    let chip = chip_text(v, f);
    let g = p.layout_no_wrap(chip.into(), FontId::proportional(14.0 * k), color);
    let chip_rect = Rect::from_min_size(pos2(rect.right() - pad - g.size().x - 28.0 * k, rect.top() + pad), vec2(g.size().x + 28.0 * k, 30.0 * k));
    p.rect_filled(chip_rect, 15.0 * k, color.gamma_multiply(0.16));
    p.rect_stroke(chip_rect, 15.0 * k, Stroke::new(1.0_f32, color), StrokeKind::Inside);
    p.galley(chip_rect.center() - g.size() * 0.5, g, color);
    let titles = (inner - chip_rect.width() - 16.0 * k).max(40.0);
    let (brand, event) = (s.brand.trim(), s.event.trim());
    let mut y = rect.top() + pad;
    if !brand.is_empty() {
        y += fit_at(&p, Align2::LEFT_TOP, pos2(rect.left() + pad, y), brand, titles, 14.0 * k, accent).height() + 4.0 * k;
    }
    if !event.is_empty() {
        y += fit_at(&p, Align2::LEFT_TOP, pos2(rect.left() + pad, y), event, titles, 24.0 * k, WHITE).height();
    }
    let header_bottom = y.max(chip_rect.bottom());
    // From the bottom up: the four figures, the progress bar, and between it and the header the time.
    let cards_h = 88.0 * k;
    let cards_top = rect.bottom() - pad - cards_h;
    let gap = 12.0 * k;
    let card_w = ((inner - gap * 3.0) / 4.0).max(10.0);
    for (i, (label, value, warn)) in figures(s, v, f).into_iter().enumerate() {
        let card = Rect::from_min_size(pos2(rect.left() + pad + i as f32 * (card_w + gap), cards_top), vec2(card_w, cards_h));
        p.rect_filled(card, 10.0 * k, PANEL);
        p.rect_stroke(card, 10.0 * k, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
        let inset = 14.0 * k;
        fit_at(&p, Align2::LEFT_TOP, card.min + vec2(inset, 12.0 * k), label, card_w - inset * 2.0, 11.0 * k, MUTED);
        fit_at(
            &p,
            Align2::LEFT_BOTTOM,
            pos2(card.left() + inset, card.bottom() - 12.0 * k),
            &value,
            card_w - inset * 2.0,
            32.0 * k,
            if warn { END } else { WHITE },
        );
    }
    let bar = Rect::from_min_size(pos2(rect.left() + pad, cards_top - 24.0 * k - 16.0 * k), vec2(inner, 16.0 * k));
    p.rect_filled(bar, 8.0 * k, Color32::from_rgb(41, 58, 64));
    if v.fill > 0.0 {
        p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * v.fill, bar.height())), 8.0 * k, color);
    }
    let room = Rect::from_min_max(pos2(rect.left() + pad, header_bottom + 8.0 * k), pos2(rect.right() - pad, bar.top() - 14.0 * k));
    let label = if f.qna {
        "Q&A"
    } else if s.count_up {
        "ELAPSED TIME"
    } else {
        "TIME REMAINING"
    };
    let digits = if matches!(v.phase, Phase::Warning | Phase::End | Phase::Over) { color } else { WHITE };
    scene::fit_text(&p, pos2(room.center().x, room.top() + 8.0 * k), label, room.width() * 0.8, 12.0 * k, MUTED);
    let size = (room.height() * 0.86).min(room.width() * 0.3).max(10.0);
    scene::fit_text(&p, room.center() + vec2(0.0, 10.0 * k), &v.text, room.width() * 0.92, size, digits);
    Rect::from_center_size(room.center(), vec2(room.width() * 0.6, room.height() * 0.8)).intersect(rect)
}

/// The small view: what the floating timer shows (digits and a thin bar), on a plain background.
fn mini(p: &Painter, rect: Rect, s: &Settings, v: &timer::View, f: &Facts) -> Rect {
    let p = p.with_clip_rect(rect);
    p.rect_filled(rect, 12, BG);
    p.rect_stroke(rect, 12, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    let status = overlay::status_text(v, s.count_up, f.running, f.started, f.qna, f.duration);
    overlay::timer(&p, rect, s, Mode::Bar, v, &status, &mut Vec::new());
    rect.shrink2(rect.size() * 0.15)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every text a frame drew.
    fn texts(output: &FullOutput) -> Vec<String> {
        fn walk(shape: &Shape, out: &mut Vec<String>) {
            match shape {
                Shape::Text(t) => out.push(t.galley.text().to_string()),
                Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        output.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    /// Whether a frame shows `want`. The small view lays its digits out one by one, so for it every character counts.
    fn shows(text: &[String], want: &str) -> bool {
        text.iter().any(|t| t == want) || want.chars().all(|c| text.iter().any(|t| *t == c.to_string()))
    }

    fn drawn(s: &Settings, format: Format, size: Vec2, clock: &Clock, now: f64) -> (Vec<String>, Rect) {
        let ctx = Context::default();
        install_fonts(&ctx, s.language, false);
        let facts = Facts { qna: clock.qna, running: clock.running(), started: clock.qna || clock.elapsed(now) > 0.0, duration: clock.duration };
        let v = clock.view(now, s.warning, s.count_up, s.overtime, s.seconds_only);
        let mut at = Rect::NOTHING;
        let out = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() }, |ctx| {
            CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
                at = paint(ui.painter(), ui.max_rect(), s, format.look(s.mode), &v, &facts, &Pictures::default());
            });
        });
        (texts(&out), at)
    }

    #[test]
    fn follow_takes_the_editors_display_and_the_others_keep_their_own() {
        for main in [Mode::Circle, Mode::Bar, Mode::Number] {
            assert_eq!(Format::Follow.look(main), Look::Stage(main));
            assert_eq!(Format::Circle.look(main), Look::Stage(Mode::Circle));
            assert_eq!(Format::Bar.look(main), Look::Stage(Mode::Bar));
            assert_eq!(Format::Number.look(main), Look::Stage(Mode::Number));
            assert_eq!(Format::Presenter.look(main), Look::Presenter);
            assert_eq!(Format::Mini.look(main), Look::Mini);
        }
        // Only digits stand still between the seconds.
        assert!(Format::Follow.moves(Mode::Circle) && !Format::Follow.moves(Mode::Number));
        assert!(Format::Circle.moves(Mode::Number) && !Format::Number.moves(Mode::Circle));
        assert!(Format::Presenter.moves(Mode::Number) && Format::Mini.moves(Mode::Number));
    }

    #[test]
    fn formats_are_saved_by_name_and_an_unknown_one_is_refused() {
        for f in Format::ALL {
            assert_eq!(serde_json::from_str::<Format>(&serde_json::to_string(&f).unwrap()).unwrap(), f);
        }
        assert_eq!(serde_json::to_string(&Format::Presenter).unwrap(), "\"Presenter\"");
        assert!(serde_json::from_str::<Format>("\"Hologram\"").is_err());
    }

    #[test]
    fn screens_open_apart_and_have_windows_of_their_own() {
        assert_ne!(cascade(1), cascade(2));
        assert_eq!(cascade(1), cascade(9), "after eight they start over at the corner");
        assert_ne!(viewport_id(1), viewport_id(2));
        assert_eq!(viewport_id(7), viewport_id(7));
        assert_ne!(viewport_id(1), ViewportId::ROOT);
    }

    #[test]
    fn every_format_draws_the_time_in_every_language_at_any_size() {
        let mut clock = Clock::new(300.0);
        clock.toggle(0.0);
        for lang in Lang::ALL {
            lang::set(lang);
            let s = Settings { language: lang, ..Settings::default() };
            for format in Format::ALL {
                for size in [vec2(1100.0, 720.0), vec2(480.0, 320.0), vec2(200.0, 90.0), vec2(1920.0, 1080.0)] {
                    let (text, at) = drawn(&s, format, size, &clock, 75.0);
                    assert!(shows(&text, "03:45"), "{lang:?} {format:?} {size:?}: {text:?}");
                    assert!(at.is_positive() && Rect::from_min_size(Pos2::ZERO, size).contains_rect(at), "{lang:?} {format:?} {size:?}: {at:?}");
                }
            }
        }
        lang::set(Lang::Ko);
    }

    #[test]
    fn screens_in_different_formats_show_the_same_clock() {
        // One clock, five looks: the same time on all of them, and a change on the clock reaches them all.
        let s = Settings::default();
        let mut clock = Clock::new(300.0);
        let all = |clock: &Clock, now: f64| -> Vec<bool> {
            let want = clock.view(now, s.warning, s.count_up, s.overtime, s.seconds_only).text;
            Format::ALL.iter().map(|f| shows(&drawn(&s, *f, vec2(960.0, 540.0), clock, now).0, &want)).collect()
        };
        assert!(all(&clock, 0.0).iter().all(|x| *x));
        clock.toggle(10.0);
        assert!(all(&clock, 70.0).iter().all(|x| *x), "running");
        clock.adjust(60.0, 70.0);
        assert!(all(&clock, 70.0).iter().all(|x| *x), "after +1 minute");
        clock.toggle(80.0);
        assert!(all(&clock, 500.0).iter().all(|x| *x), "paused");
        clock.reset(300.0);
        assert!(all(&clock, 500.0).iter().all(|x| *x), "after a reset");
    }

    #[test]
    fn the_presenter_view_shows_the_figures_staff_need() {
        let mut s = Settings::default();
        let mut clock = Clock::new(600.0);
        clock.toggle(0.0);
        let at = |s: &Settings, clock: &Clock, now| drawn(s, Format::Presenter, vec2(960.0, 540.0), clock, now).0;
        let text = at(&s, &clock, 125.0);
        for want in ["10:00", "02:05", "07:55", "01:00", "설정 시간", "경과 시간", "남은 시간", "주의 알림", "진행 중", &s.brand, &s.event]
        {
            assert!(text.iter().any(|t| t == want), "{want}: {text:?}");
        }
        // With Q&A on, the last figure is what comes next.
        s.qna_enabled = true;
        let text = at(&s, &clock, 125.0);
        assert!(text.iter().any(|t| t == "Q&A 예정") && text.iter().any(|t| t == "05:00"), "{text:?}");
        // Over time: the figure turns into what has been used beyond the set time.
        let text = at(&s, &clock, 640.0);
        assert!(text.iter().any(|t| t == "초과 시간") && text.iter().any(|t| t == "+00:40") && text.iter().any(|t| t == "시간 초과"), "{text:?}");
        // Paused and not yet started.
        let mut paused = Clock::new(600.0);
        paused.toggle(0.0);
        paused.toggle(100.0);
        assert!(at(&s, &paused, 900.0).iter().any(|t| t == "일시정지"));
        assert!(at(&s, &Clock::new(600.0), 0.0).iter().any(|t| t == "준비"));
    }

    #[test]
    fn the_figures_follow_the_clock() {
        let s = Settings::default();
        let mut c = Clock::new(300.0);
        let facts = |c: &Clock, now: f64| Facts { qna: c.qna, running: c.running(), started: c.elapsed(now) > 0.0, duration: c.duration };
        let values = |c: &Clock, now: f64| figures(&s, &c.view(now, 60.0, false, true, false), &facts(c, now)).map(|f| f.1);
        assert_eq!(values(&c, 0.0), ["05:00", "00:00", "05:00", "01:00"]);
        c.toggle(0.0);
        assert_eq!(values(&c, 100.5), ["05:00", "01:40", "03:20", "01:00"]);
        assert_eq!(values(&c, 300.2), ["05:00", "05:00", "00:00", "01:00"]);
        assert_eq!(values(&c, 330.0), ["05:00", "05:30", "+00:30", "01:00"]);
        let off = Settings { warning: 0.0, ..Settings::default() };
        assert_eq!(figures(&off, &c.view(10.0, 0.0, false, true, false), &facts(&c, 10.0))[3].1, "—");
        // The warning colour shows only for time that is up.
        assert!(!figures(&s, &c.view(100.0, 60.0, false, true, false), &facts(&c, 100.0))[2].2);
        assert!(figures(&s, &c.view(305.0, 60.0, false, true, false), &facts(&c, 305.0))[2].2);
    }

    #[test]
    fn the_chip_says_where_the_timer_stands() {
        let mut c = Clock::new(300.0);
        let facts = |c: &Clock, now: f64| Facts { qna: c.qna, running: c.running(), started: c.elapsed(now) > 0.0, duration: c.duration };
        let chip = |c: &Clock, now: f64| chip_text(&c.view(now, 60.0, false, true, false), &facts(c, now));
        assert_eq!(chip(&c, 0.0), "준비");
        c.toggle(0.0);
        assert_eq!(chip(&c, 10.0), "진행 중");
        assert_eq!(chip(&c, 250.0), "곧 종료");
        assert_eq!(chip(&c, 300.5), "시간 종료");
        assert_eq!(chip(&c, 305.0), "시간 초과");
        c.toggle(100.0);
        assert_eq!(chip(&c, 120.0), "일시정지");
        c.reset(300.0);
        c.switch_qna(120.0, false, 0.0);
        assert_eq!(chip(&c, 5.0), "질의응답");
    }
}
