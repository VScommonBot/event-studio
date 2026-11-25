// (A test sets up the settings it is about field by field.)
#![allow(clippy::field_reassign_with_default)]
use super::*;
fn fixture(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("checks").join(name);
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn settings_roundtrip() {
    let mut s = Settings::default();
    s.brand = "테스트 브랜드".into();
    s.event = "한글 행사명 테스트".into();
    s.logo = "assets/logo.png".into();
    s.mode = Mode::Number;
    s.qna_enabled = true;
    let text = serde_json::to_string(&s).unwrap();
    let r: Settings = serde_json::from_str(&text).unwrap();
    assert_eq!(r.event, s.event);
    assert_eq!(r.logo, s.logo);
    assert_eq!(r.mode, Mode::Number);
    assert!(r.qna_enabled);
}
#[test]
fn settings_validation() {
    let mut s: Settings = serde_json::from_str(r#"{"duration":-4,"warning":999999,"logo_position":88,"volume":3.0,"dim":-1.0}"#).unwrap();
    s.normalize();
    assert_eq!(s.duration, 0.0);
    assert_eq!(s.warning, timer::MAX_SECONDS);
    assert_eq!(s.logo_position, 5);
    assert_eq!(s.volume, 1.0);
    assert_eq!(s.dim, 0.0);
    assert_eq!(s.qna_duration, 300.0);
}
#[test]
fn image_load_transparency() {
    let root = fixture("image-test");
    let path = root.join("한글 로고.png");
    let mut image = image::RgbaImage::new(120, 60);
    image.put_pixel(20, 20, image::Rgba([132, 226, 190, 128]));
    image.save(&path).unwrap();
    let decoded = load_image(&path).unwrap();
    assert_eq!(decoded.dimensions(), (120, 60));
    assert_eq!(decoded.get_pixel(20, 20).0, [132, 226, 190, 128]);
}
#[test]
fn invalid_image_rejected() {
    let root = fixture("image-test");
    let path = root.join("not-an-image.png");
    std::fs::write(&path, b"not an image").unwrap();
    assert!(load_image(&path).is_err());
}
#[test]
fn settings_atomic_save_and_backup() {
    let root = fixture("settings-test");
    let mut app = Studio::bare(Settings::default(), root.clone(), None);
    app.s.brand = "first".into();
    app.save();
    assert_eq!(serde_json::from_str::<Settings>(&std::fs::read_to_string(root.join("settings.json")).unwrap()).unwrap().brand, "first");
    app.s.brand = "second".into();
    app.save();
    assert_eq!(serde_json::from_str::<Settings>(&std::fs::read_to_string(root.join("settings.backup.json")).unwrap()).unwrap().brand, "first");
    assert_eq!(serde_json::from_str::<Settings>(&std::fs::read_to_string(root.join("settings.json")).unwrap()).unwrap().brand, "second");
    assert!(!root.join("settings.pending.json").exists());
}
// A picture larger than its slot needs is made smaller in its own proportions; a logo's see-through surroundings do not tint it.
#[test]
fn large_pictures_are_made_smaller_and_keep_their_colours() {
    assert_eq!(fit_within(image::RgbaImage::new(8000, 400), BACKGROUND_SIDE).dimensions(), (3840, 192));
    assert_eq!(fit_within(image::RgbaImage::new(100, 5000), LOGO_SIDE).dimensions(), (41, 2048));
    assert_eq!(fit_within(image::RgbaImage::new(8192, 1), BACKGROUND_SIDE).dimensions(), (3840, 1));
    assert_eq!(fit_within(image::RgbaImage::new(3840, 216), BACKGROUND_SIDE).dimensions(), (3840, 216), "a picture that fits stays as it is");
    assert_eq!((side_for(0), side_for(1), side_for(2)), (BACKGROUND_SIDE, LOGO_SIDE, LOGO_SIDE));
    // Left half see-through green, right half solid red: at the seam the colour is red, only fainter.
    let logo = image::RgbaImage::from_fn(4096, 64, |x, _| if x < 2048 { image::Rgba([0, 255, 0, 0]) } else { image::Rgba([200, 30, 40, 255]) });
    let small = fit_within(logo, LOGO_SIDE);
    assert_eq!(small.dimensions(), (2048, 32));
    assert_eq!(small.get_pixel(0, 10).0[3], 0);
    assert_eq!(small.get_pixel(2047, 10).0, [200, 30, 40, 255]);
    for x in 1020..1028 {
        let p = small.get_pixel(x, 10).0;
        if p[3] > 0 {
            assert!(p[1] <= 31 && p[0] >= 199, "{x}: {p:?}");
        }
    }
    // An opaque picture keeps its colours exactly.
    let flat = fit_within(image::RgbaImage::from_pixel(8192, 16, image::Rgba([12, 34, 56, 255])), BACKGROUND_SIDE);
    assert!(flat.pixels().all(|p| p.0 == [12, 34, 56, 255]));
}
// A settings file that cannot be read: the backup opens instead, and stays the backup.
fn clean_fixture(name: &str) -> PathBuf {
    let root = fixture(name);
    for file in ["settings.json", "settings.backup.json", "settings.pending.json", UNREADABLE_NAME] {
        let _ = std::fs::remove_file(root.join(file));
    }
    root
}
fn saved_brand(root: &Path, file: &str) -> String {
    serde_json::from_str::<Settings>(&std::fs::read_to_string(root.join(file)).unwrap()).unwrap().brand
}
#[test]
fn unreadable_settings_open_from_the_backup_and_never_take_its_place() {
    let root = clean_fixture("recover-test");
    let mut app = Studio::bare(Settings::default(), root.clone(), None);
    for brand in ["first", "second"] {
        app.s.brand = brand.into();
        app.save();
    }
    std::fs::write(root.join("settings.json"), "{ not settings").unwrap();
    let (s, warning) = load_settings(&root);
    assert_eq!(s.brand, "first");
    assert_eq!(warning.as_deref(), Some("저장 설정을 읽지 못해 직전에 저장한 설정으로 열었습니다."));
    // Opening changed nothing on disk; the next save keeps the good backup and puts the unreadable file aside.
    assert_eq!(std::fs::read_to_string(root.join("settings.json")).unwrap(), "{ not settings");
    assert!(!root.join(UNREADABLE_NAME).exists());
    let mut app = Studio::bare(s, root.clone(), None);
    app.s.brand = "third".into();
    app.save();
    assert_eq!(saved_brand(&root, "settings.backup.json"), "first");
    assert_eq!(std::fs::read_to_string(root.join(UNREADABLE_NAME)).unwrap(), "{ not settings");
    assert_eq!(saved_brand(&root, "settings.json"), "third");
    // From then on a save backs up what it replaces, as before.
    app.s.brand = "fourth".into();
    app.save();
    assert_eq!((saved_brand(&root, "settings.backup.json"), saved_brand(&root, "settings.json")), ("third".into(), "fourth".into()));
}
#[test]
fn settings_from_a_build_with_values_this_one_lacks_open_from_the_backup() {
    let root = clean_fixture("recover-newer-test");
    std::fs::write(root.join("settings.backup.json"), r#"{"brand":"kept","mode":"Bar"}"#).unwrap();
    std::fs::write(root.join("settings.json"), r#"{"brand":"newer","mode":"Dial"}"#).unwrap();
    let (s, warning) = load_settings(&root);
    assert_eq!((s.brand.as_str(), s.mode, warning.is_some()), ("kept", Mode::Bar, true));
}
#[test]
fn without_a_readable_backup_or_without_settings_it_is_a_first_start() {
    let root = clean_fixture("recover-none-test");
    let first = Settings::first(Lang::system());
    // No settings file: a first start without a warning, even if a backup was left behind (deleting settings.json starts afresh).
    std::fs::write(root.join("settings.backup.json"), r#"{"brand":"left behind"}"#).unwrap();
    let (s, warning) = load_settings(&root);
    assert_eq!((s.brand, warning), (first.brand.clone(), None));
    // Neither file can be read: a first start, and the person is told.
    for file in ["settings.json", "settings.backup.json"] {
        std::fs::write(root.join(file), [0xff, 0xfe, 0x00]).unwrap();
    }
    let (s, warning) = load_settings(&root);
    assert_eq!(s.brand, first.brand);
    assert_eq!(warning.as_deref(), Some("저장 설정을 읽지 못해 기본 설정으로 열었습니다."));
    // Saved values out of range are still brought into range.
    std::fs::write(root.join("settings.json"), r#"{"volume":3.0,"logo_position":88}"#).unwrap();
    let (s, _) = load_settings(&root);
    assert_eq!((s.volume, s.logo_position), (1.0, 5));
}
// How often the window is drawn again: only when what it shows changes, not 60 times a second throughout.
fn rested(root: &str) -> Studio {
    let mut app = Studio::bare(Settings::default(), fixture(root), None);
    app.frames = 1000;
    app.activity = Instant::now().checked_sub(Duration::from_secs(4)).expect("the computer has been on for four seconds");
    app
}
fn wait_of(app: &Studio) -> f64 {
    app.next_frame_in().expect("a frame is due").as_secs_f64()
}
#[test]
fn a_window_where_nothing_moves_is_not_drawn_again_by_itself() {
    let mut app = rested("frames-test");
    assert_eq!(app.next_frame_in(), None);
    // The mouse just moved: once more when the hover buttons are due to hide.
    app.activity = Instant::now();
    let wait = wait_of(&app);
    assert!(wait > 2.0 && wait <= CONTROLS_IDLE + 0.01, "{wait}");
    // Each output screen has hover buttons of its own.
    let mut app = rested("frames-test");
    assert!(app.open_screen(Format::Number));
    app.screens[0].activity = Instant::now();
    assert!(wait_of(&app) <= CONTROLS_IDLE + 0.01);
    app.screens[0].activity = Instant::now().checked_sub(Duration::from_secs(4)).expect("the computer has been on for four seconds");
    app.frames += SETTLE_FRAMES;
    assert_eq!(app.next_frame_in(), None);
    // A changed setting: when it is due to be saved.
    let mut app = rested("frames-test");
    app.dirty_since = Some(Instant::now());
    let wait = wait_of(&app);
    assert!(wait > 0.3 && wait <= 0.61, "{wait}");
}
#[test]
fn a_running_timer_is_drawn_at_each_second_and_its_graph_in_small_steps() {
    let mut app = rested("frames-test");
    app.s.mode = Mode::Number;
    app.toggle();
    let wait = wait_of(&app);
    assert!(wait > 0.5 && wait <= 1.01, "{wait}");
    // The ring of a five-minute timer: 3000 steps, one every tenth of a second.
    app.s.mode = Mode::Circle;
    let wait = wait_of(&app);
    assert!((0.09..=0.11).contains(&wait), "{wait}");
    // A long timer never less than four times a second, a short one never more than every frame.
    app.clock.duration = 7200.0;
    let wait = wait_of(&app);
    assert!((0.24..=0.26).contains(&wait), "{wait}");
    app.s.mode = Mode::Bar;
    app.clock.duration = 20.0;
    let wait = wait_of(&app);
    assert!((0.016..=0.02).contains(&wait), "{wait}");
    // Past the end the graph stands still: back to once a second for the overtime.
    app.clock.accumulated = 40.0;
    let wait = wait_of(&app);
    assert!(wait > 0.5 && wait <= 1.01, "{wait}");
    // Paused: nothing moves.
    app.toggle();
    assert_eq!(app.next_frame_in(), None);
}
#[test]
fn the_floating_timer_a_drag_and_a_check_keep_their_own_pace() {
    // The floating timer is put back on top once a second, and blinks twice a second at the end.
    // (On macOS it also follows the pointer, twenty times a second.)
    let most = |usual: f64| if win::FOLLOWS_POINTER { POINTER_POLL } else { usual } + 0.01;
    let least = |usual: f64| if win::FOLLOWS_POINTER { 0.04 } else { usual };
    let mut app = rested("frames-test");
    app.overlay = true;
    app.last_topmost = app.now();
    let wait = wait_of(&app);
    assert!(wait > least(0.5) && wait <= most(TOPMOST_EVERY), "{wait}");
    app.flash_since = Some(Instant::now());
    let wait = wait_of(&app);
    assert!(wait > least(0.2) && wait <= most(0.5), "{wait}");
    // Every frame: a drag, a window being placed or settling, a check's fixed state.
    let mut app = rested("frames-test");
    app.gesture = Some(Gesture { kind: GestureKind::Move, cursor: (0, 0), rect: win::PxRect { x: 0, y: 0, w: 360, h: 200 }, moved: false });
    assert_eq!(app.next_frame_in(), Some(FRAME));
    let mut app = rested("frames-test");
    app.geometry = Some((win::PxRect { x: 0, y: 0, w: 360, h: 200 }, app.frames));
    assert_eq!(app.next_frame_in(), Some(FRAME));
    let mut app = rested("frames-test");
    app.settle();
    assert_eq!(app.next_frame_in(), Some(FRAME));
    app.frames += SETTLE_FRAMES;
    assert_eq!(app.next_frame_in(), None);
    let mut app = rested("frames-test");
    app.steady = true;
    assert_eq!(app.next_frame_in(), Some(FRAME));
    assert_eq!(Studio::bare(Settings::default(), fixture("frames-test"), None).next_frame_in(), Some(FRAME), "the first frames of a start");
}
#[test]
fn older_settings_get_floating_defaults() {
    let s: Settings = serde_json::from_str(r#"{"brand":"B","mode":"Bar"}"#).unwrap();
    assert!(!s.overlay_start && s.overlay_top_right && !s.overlay_scene && !s.hotkeys);
    assert_eq!(s.overlay_backdrop, 0.0);
    assert!(s.overlay_rect.is_none());
}
#[test]
fn floating_settings_limits() {
    let mut s: Settings = serde_json::from_str(r#"{"overlay_backdrop":3.0,"overlay_rect":[10,10,0,5]}"#).unwrap();
    s.normalize();
    assert_eq!(s.overlay_backdrop, BACKDROP_MAX);
    assert!(s.overlay_rect.is_none());
    let mut s: Settings = serde_json::from_str(r#"{"overlay_backdrop":-1.0,"overlay_rect":[10,10,360,200]}"#).unwrap();
    s.normalize();
    assert_eq!(s.overlay_backdrop, 0.0);
    assert_eq!(s.overlay_rect, Some([10, 10, 360, 200]));
}
#[test]
fn floating_status_lines() {
    let mut c = Clock::new(300.0);
    let v = c.view(0.0, 60.0, false, true, false);
    assert_eq!(overlay::status_text(&v, false, false, false, false, 300.0), "");
    assert_eq!(overlay::status_text(&v, true, false, false, false, 300.0), "목표 05:00");
    c.toggle(0.0);
    let v = c.view(250.0, 60.0, false, true, false);
    assert_eq!(overlay::status_text(&v, false, true, true, false, 300.0), "곧 종료");
    let v = c.view(300.5, 60.0, false, true, false);
    assert_eq!(overlay::status_text(&v, false, true, true, false, 300.0), "시간 종료");
    let v = c.view(302.0, 60.0, false, true, false);
    assert_eq!(overlay::status_text(&v, false, true, true, false, 300.0), "시간 초과");
    c.toggle(100.0);
    let v = c.view(100.0, 60.0, false, true, false);
    assert_eq!(overlay::status_text(&v, false, false, true, false, 300.0), "일시정지");
}
// The floating timer driven by simulated pointer events (never the real mouse).
fn float_frame(ctx: &Context, app: &mut Studio, events: Vec<Event>) {
    let _ =
        ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, FLOAT_SIZE)), events, ..Default::default() }, |ctx| app.floating(ctx));
}
fn float_click(ctx: &Context, app: &mut Studio, at: Pos2) {
    float_frame(ctx, app, vec![Event::PointerMoved(at)]);
    for pressed in [true, false] {
        float_frame(ctx, app, vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
    }
}
fn float_press(ctx: &Context, app: &mut Studio, cmd: controls::Cmd) {
    let at = app.float_buttons.iter().find(|b| b.0 == cmd).map(|b| b.1.center()).expect("button shown");
    float_click(ctx, app, at);
}
#[test]
fn floating_timer_shows_buttons_on_hover_and_they_work() {
    let ctx = Context::default();
    install_fonts(&ctx, Lang::Ko, false);
    let mut app = Studio::bare(Settings::default(), fixture("float-test"), None);
    app.overlay = true;
    for _ in 0..5 {
        float_frame(&ctx, &mut app, vec![]);
    }
    assert!(app.float_buttons.is_empty(), "no buttons until the mouse is over it");
    for i in 0..30 {
        float_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(120.0 + i as f32, 90.0))]);
    }
    assert_eq!(
        app.float_buttons.iter().map(|b| b.0).collect::<Vec<_>>(),
        [controls::Cmd::Reset, controls::Cmd::Play, controls::Cmd::Pause, controls::Cmd::Editor, controls::Cmd::Quit]
    );
    assert!(app.float_buttons.iter().all(|b| Rect::from_min_size(Pos2::ZERO, FLOAT_SIZE).contains_rect(b.1)), "buttons stay inside the window");
    float_press(&ctx, &mut app, controls::Cmd::Play);
    assert!(app.clock.running(), "재시작/시작 starts");
    float_press(&ctx, &mut app, controls::Cmd::Pause);
    assert!(!app.clock.running(), "멈춤 pauses");
    float_press(&ctx, &mut app, controls::Cmd::Play);
    assert!(app.clock.running(), "재시작 resumes");
    float_press(&ctx, &mut app, controls::Cmd::Reset);
    assert!(!app.clock.running() && app.clock.elapsed(app.now()) == 0.0, "리셋 goes back to the full time, stopped");
    float_press(&ctx, &mut app, controls::Cmd::Editor);
    assert!(!app.overlay, "the gear goes back to the editor");
}
#[test]
fn floating_buttons_hide_when_the_mouse_leaves() {
    let ctx = Context::default();
    install_fonts(&ctx, Lang::Ko, false);
    let mut app = Studio::bare(Settings::default(), fixture("float-test"), None);
    app.overlay = true;
    for i in 0..30 {
        float_frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(60.0 + i as f32, 60.0))]);
    }
    assert!(!app.float_buttons.is_empty());
    float_frame(&ctx, &mut app, vec![Event::PointerGone]);
    for _ in 0..30 {
        float_frame(&ctx, &mut app, vec![]);
    }
    assert!(app.float_buttons.is_empty());
}
#[test]
fn floating_timer_tiny_window_keeps_buttons_inside() {
    let ctx = Context::default();
    install_fonts(&ctx, Lang::Ko, false);
    let mut app = Studio::bare(Settings::default(), fixture("float-test"), None);
    app.overlay = true;
    app.force_controls = true;
    for _ in 0..30 {
        let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, FLOAT_MIN)), ..Default::default() }, |ctx| app.floating(ctx));
    }
    assert_eq!(app.float_buttons.len(), 5);
    assert!(app.float_buttons.iter().all(|b| Rect::from_min_size(Pos2::ZERO, FLOAT_MIN).expand(1.0).contains_rect(b.1)));
}
fn painted_parts(mode: Mode, backdrop: f32) -> Vec<overlay::Solid> {
    let ctx = Context::default();
    install_fonts(&ctx, Lang::Ko, false);
    let mut s = Settings::default();
    s.mode = mode;
    s.overlay_backdrop = backdrop;
    let v = Clock::new(300.0).view(0.0, 60.0, false, true, false);
    let rect = Rect::from_min_size(Pos2::ZERO, FLOAT_SIZE);
    let mut solid = Vec::new();
    let _ = ctx.run(RawInput { screen_rect: Some(rect), ..Default::default() }, |ctx| {
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            solid = overlay::paint(ui.painter(), rect, &s, &v, "", &Pictures::default());
        });
    });
    solid
}
// What the floating timer paints in a window of `size` with these settings.
fn painted_with(s: &Settings, size: Vec2) -> Vec<overlay::Solid> {
    let ctx = Context::default();
    install_fonts(&ctx, Lang::Ko, false);
    let v = Clock::new(300.0).view(0.0, 60.0, false, true, false);
    let rect = Rect::from_min_size(Pos2::ZERO, size);
    let mut solid = Vec::new();
    let _ = ctx.run(RawInput { screen_rect: Some(rect), ..Default::default() }, |ctx| {
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            solid = overlay::paint(ui.painter(), rect, s, &v, "", &Pictures::default());
        });
    });
    solid
}
fn ring_of(solid: &[overlay::Solid]) -> (Pos2, f32) {
    solid
        .iter()
        .find_map(|a| match a {
            overlay::Solid::Disc(c, r) => Some((*c, *r)),
            _ => None,
        })
        .expect("a ring")
}
// "배경이 투명하게": with brand, event name and logos shown, the floating timer still has no background.
#[test]
fn floating_timer_with_titles_keeps_the_background_see_through() {
    let size = vec2(375.0, 495.0);
    let mut s = Settings::default();
    s.brand = "브랜드".into();
    s.event = "행사 이름".into();
    s.subtitle = "세션".into();
    let plain = painted_with(&s, size);
    s.overlay_scene = true;
    let titled = painted_with(&s, size);
    for corner in [pos2(4.0, 4.0), pos2(371.0, 4.0), pos2(4.0, 491.0), pos2(371.0, 491.0), pos2(8.0, 247.0)] {
        assert!(!titled.iter().any(|a| a.contains(corner)), "nothing is painted at {corner:?}");
    }
    assert!(!titled.iter().any(|a| matches!(a,overlay::Solid::Rect(r) if r.width()>=size.x-1.0&&r.height()>=size.y-1.0)), "no sheet behind it all");
    assert_eq!(titled.len(), plain.len() + 3, "brand, event name and subtitle are painted");
    let (plain_ring, titled_ring) = (ring_of(&plain), ring_of(&titled));
    assert!(titled_ring.0.y > plain_ring.0.y, "the ring moves down to make room");
    let titles_bottom = titled
        .iter()
        .filter_map(|a| match a {
            overlay::Solid::Rect(r) => Some(r.bottom()),
            _ => None,
        })
        .fold(0.0, f32::max);
    assert!(titles_bottom <= titled_ring.0.y - titled_ring.1 + 8.0, "titles end where the ring starts");
    assert!(titles_bottom <= size.y * 0.5);
    // Nothing to show: the same as without the option.
    s.brand.clear();
    s.event.clear();
    s.subtitle = "  ".into();
    assert_eq!(painted_with(&s, size), plain);
    // A backdrop is the only thing that fills the window, and only when asked for.
    s.overlay_backdrop = 0.3;
    assert!(painted_with(&s, size).iter().any(|a| a.contains(pos2(4.0, 4.0))));
}
#[test]
fn settings_from_before_the_see_through_stage_start_see_through() {
    let mut s: Settings = serde_json::from_str(r#"{"overlay_scene":true,"overlay_backdrop":0.07}"#).unwrap();
    s.normalize();
    s.migrate();
    assert_eq!(s.overlay_backdrop, 0.0);
    assert_eq!(s.schema, SCHEMA);
    assert!(s.overlay_scene);
    let mut s: Settings = serde_json::from_str(r#"{"schema":2,"overlay_scene":true,"overlay_backdrop":0.25}"#).unwrap();
    s.normalize();
    s.migrate();
    assert_eq!(s.overlay_backdrop, 0.25);
    let mut s: Settings = serde_json::from_str(r#"{"overlay_scene":false,"overlay_backdrop":0.25}"#).unwrap();
    s.normalize();
    s.migrate();
    assert_eq!(s.overlay_backdrop, 0.25);
    let again: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(again.schema, SCHEMA);
}
// As on a per-pixel-alpha layered window, only painted pixels take the mouse.
#[test]
fn floating_timer_only_painted_parts_take_the_mouse() {
    let centre = pos2(FLOAT_SIZE.x / 2.0, FLOAT_SIZE.y / 2.0);
    let corner = pos2(6.0, 6.0);
    for mode in [Mode::Circle, Mode::Bar, Mode::Number] {
        let solid = painted_parts(mode, 0.0);
        assert!(solid.iter().any(|a| a.contains(centre)), "{mode:?}: the time takes the mouse");
        assert!(!solid.iter().any(|a| a.contains(corner)), "{mode:?}: an empty corner lets it through");
    }
    let solid = painted_parts(Mode::Circle, 0.4);
    assert!(solid.iter().any(|a| a.contains(corner)), "a backdrop is paint too");
    let px = overlay::Solid::Disc(pos2(100.0, 50.0), 40.0).to_px(1.5);
    assert_eq!(px, win::HitShape::Ellipse(win::PxRect { x: 85, y: 10, w: 130, h: 130 }));
}
#[test]
fn hover_buttons_drive_the_clock() {
    let mut app = Studio::bare(Settings::default(), fixture("hover-test"), None);
    let ctx = Context::default();
    app.apply_cmd(controls::Cmd::Pause, &ctx);
    assert!(!app.clock.running());
    app.apply_cmd(controls::Cmd::Play, &ctx);
    assert!(app.clock.running());
    app.apply_cmd(controls::Cmd::Play, &ctx);
    assert!(app.clock.running());
    app.apply_cmd(controls::Cmd::Pause, &ctx);
    assert!(!app.clock.running());
    app.clock.accumulated = 42.0;
    app.flash_since = Some(Instant::now());
    app.apply_cmd(controls::Cmd::Reset, &ctx);
    assert!(!app.clock.running() && app.clock.elapsed(app.now()) == 0.0 && app.flash_since.is_none());
}
// Languages: the saved choice, the first start, and whether each language's fonts can draw its texts.
#[test]
fn language_is_saved_and_older_settings_stay_korean() {
    let old: Settings = serde_json::from_str(r#"{"brand":"B","mode":"Bar"}"#).unwrap();
    assert_eq!(old.language, Lang::Ko);
    let mut s = Settings::default();
    assert_eq!(s.language, Lang::Ko);
    s.language = Lang::Ja;
    let text = serde_json::to_string(&s).unwrap();
    assert!(text.contains(r#""language":"ja""#));
    assert_eq!(serde_json::from_str::<Settings>(&text).unwrap().language, Lang::Ja);
    let odd: Settings = serde_json::from_str(r#"{"language":"xx","brand":"B"}"#).unwrap();
    assert_eq!((odd.language, odd.brand.as_str()), (Lang::Ko, "B"));
    let first = Settings::first(Lang::Zh);
    assert_eq!((first.language, first.event.as_str(), first.duration), (Lang::Zh, "开启新可能的时刻", timer::DEFAULT_SECONDS));
    assert_eq!(Settings::first(Lang::Ko).event, Settings::default().event);
}
#[test]
fn switching_language_keeps_what_the_person_typed() {
    // The placeholder event name follows the language; a typed one stays. (Checked without switching the
    // language itself: that is one switch for the whole program, and the other tests read Korean.)
    assert_eq!(placeholder_in(FIRST_EVENT, Lang::Ko, Lang::En), Some("A time that opens new possibilities"));
    assert_eq!(placeholder_in("A time that opens new possibilities", Lang::En, Lang::Ja), Some("新しい可能性をひらく時間"));
    assert_eq!(placeholder_in("新しい可能性をひらく時間", Lang::Ja, Lang::Ko), Some(FIRST_EVENT));
    assert_eq!(placeholder_in("우리 행사", Lang::Ko, Lang::En), None);
    assert_eq!(placeholder_in(FIRST_EVENT, Lang::En, Lang::Ja), None, "Korean wording typed while English is on is the person's own text");
}
#[test]
fn every_language_can_draw_its_own_texts() {
    for language in Lang::ALL {
        if language_faces(language).0.iter().all(|f| system_font(*f).is_none()) {
            eprintln!("{language:?}: its fonts are not on this computer, not checked");
            continue;
        }
        let ctx = Context::default();
        install_fonts(&ctx, language, false);
        let _ = ctx.run(RawInput::default(), |ctx| {
            ctx.fonts(|f| {
                for family in [FontFamily::Proportional, bold()] {
                    let font = FontId::new(14.0, family);
                    for row in crate::strings::TABLE {
                        let text = lang::tr(language, row[0]);
                        let missing: String = text.chars().filter(|c| !c.is_control() && !f.has_glyphs(&font, &c.to_string())).collect();
                        assert!(missing.is_empty(), "{language:?}: no glyph for {missing:?} in {text:?}");
                    }
                    // The language switch shows every language under its own name.
                    for l in Lang::ALL {
                        assert!(f.has_glyphs(&font, l.native_name()), "{language:?} cannot draw {}", l.native_name());
                    }
                }
                assert!(f.has_glyphs(&FontId::proportional(14.0), "🌐"));
            })
        });
    }
}
#[test]
fn a_missing_or_odd_font_file_is_passed_over() {
    assert!(system_font(("no-such-font.ttf", Nth(0))).is_none());
    assert!(system_font(("no-such-font.ttc", Pick::Like("", 400))).is_none());
    // A face a file does not have: the single font file of Malgun Gothic has only face 0.
    if system_font(("malgun.ttf", Nth(0))).is_some() {
        assert!(system_font(("malgun.ttf", Nth(1))).is_none());
    }
    if let Some(collection) = system_font(("msyh.ttc", Nth(1))) {
        assert_eq!(collection.index, 1);
        assert!(system_font(("msyh.ttc", Nth(9))).is_none());
    }
    // Bytes that are not a font have no face to pick.
    assert_eq!(face_like(b"not a font at all", "", 400), None);
}
// On a Mac every language has to find its fonts: the lists name macOS' own files.
/// What a folder of fonts holds, face by face: printed when a font is not found, to see what this macOS has.
#[cfg(target_os = "macos")]
fn fonts_in(folder: &str) -> String {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).map(|list| list.flatten().map(|entry| entry.path()).collect()).unwrap_or_default();
    files.sort();
    files
        .iter()
        .filter(|file| file.extension().is_some_and(|e| e == "ttc" || e == "ttf" || e == "otf"))
        .map(|file| {
            let bytes = std::fs::read(file).unwrap_or_default();
            let count = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
            let faces: Vec<String> = (0..count)
                .map(|index| match ttf_parser::Face::parse(&bytes, index) {
                    Ok(face) => format!(
                        "{index}:{:?}/{}{}",
                        postscript_names(&face),
                        face.weight().to_number(),
                        if face.is_italic() || face.is_oblique() { "/slanted" } else { "" }
                    ),
                    Err(e) => format!("{index}:unreadable ({e})"),
                })
                .collect();
            format!("{} ({} bytes): {}\n", file.display(), bytes.len(), faces.join(" "))
        })
        .collect()
}
#[cfg(target_os = "macos")]
#[test]
fn every_language_finds_its_fonts_on_macos() {
    let inventory = || format!("{}{}", fonts_in("/System/Library/Fonts"), fonts_in("/System/Library/Fonts/Supplemental"));
    for language in Lang::ALL {
        let (regular, heavy) = language_faces(language);
        for (kind, faces) in [("regular", regular), ("bold", heavy)] {
            let found = faces.iter().find(|face| system_font(**face).is_some());
            eprintln!("{language:?} {kind}: {found:?}");
            assert!(found.is_some(), "{language:?} {kind}: none of {faces:?}\n{}", inventory());
            // The first of a list is the font the language is meant to have; the others only stand in for it.
            // (Chinese has PingFang first, which macOS 26 no longer keeps with the other fonts: there Hiragino Sans GB is it.)
            let own = if language == Lang::Zh && !Path::new(faces[0].0).exists() { faces.get(1) } else { faces.first() };
            assert_eq!(found, own, "{language:?} {kind}: its own font was not found\n{}", inventory());
        }
    }
    for (kind, faces) in [("digits", DIGIT_FACES), ("wide", WIDE_FACES)] {
        assert!(faces.iter().any(|face| system_font(*face).is_some()), "{kind}: none of {faces:?}");
    }
    // A weight is found by name: the bold face of a collection is not its regular one.
    let face = |faces: &[Face]| faces.iter().find_map(|face| system_font(*face).map(|data| (face.0, data.index)));
    assert_ne!(face(KOREAN_FACES.0), face(KOREAN_FACES.1));
}
// A macOS app keeps what it saves in Application Support; a program on its own, beside itself.
#[test]
fn a_macos_app_keeps_its_settings_in_application_support() {
    let home = Path::new("/Users/someone");
    assert_eq!(
        bundle_data_folder(Path::new("/Applications/Event Studio.app/Contents/MacOS"), Some(home)),
        Some(PathBuf::from("/Users/someone/Library/Application Support/Event Studio"))
    );
    assert_eq!(bundle_data_folder(Path::new("/Users/someone/tools"), Some(home)), None);
    assert_eq!(bundle_data_folder(Path::new("/Applications/Event Studio.app/Contents/MacOS"), None), None);
}
/// One frame of only the language box, as the editor and the setup window show it.
fn picker_frame(ctx: &Context, picked: &mut Lang, events: Vec<Event>) -> FullOutput {
    ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(420.0, 320.0))), events, ..Default::default() }, |ctx| {
        CentralPanel::default().show(ctx, |ui| language_picker(ui, picked));
    })
}
/// A click, and the frame after things have settled.
fn picker_click(ctx: &Context, picked: &mut Lang, at: Pos2) -> FullOutput {
    picker_frame(ctx, picked, vec![Event::PointerMoved(at)]);
    for pressed in [true, false] {
        picker_frame(ctx, picked, vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
    }
    picker_frame(ctx, picked, vec![]);
    picker_frame(ctx, picked, vec![])
}
/// The middle of a control, from what egui tells a screen reader about the frame: the test presses what is really drawn.
fn spot(frame: &FullOutput, is: impl Fn(&egui::accesskit::Node) -> bool) -> Option<Pos2> {
    frame
        .platform_output
        .accesskit_update
        .as_ref()?
        .nodes
        .iter()
        .find(|(_, node)| is(node))
        .and_then(|(_, node)| node.bounds())
        .map(|b| pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
}
#[test]
fn the_language_box_lists_every_language_and_a_click_picks_it() {
    let ctx = Context::default();
    ctx.enable_accesskit();
    install_fonts(&ctx, Lang::Ko, false);
    let mut picked = Lang::Ko;
    for target in [Lang::Ja, Lang::En, Lang::Zh, Lang::Ko] {
        picker_frame(&ctx, &mut picked, vec![]);
        let closed = picker_frame(&ctx, &mut picked, vec![]);
        assert!(
            Lang::ALL.iter().all(|l| spot(&closed, |n| n.label() == Some(l.native_name())).is_none()),
            "the list is shut until the box is pressed"
        );
        let the_box = spot(&closed, |n| n.role() == egui::accesskit::Role::ComboBox).expect("the language box is drawn");
        let open = picker_click(&ctx, &mut picked, the_box);
        let rows: Vec<Pos2> = Lang::ALL
            .iter()
            .map(|l| spot(&open, |n| n.label() == Some(l.native_name())).unwrap_or_else(|| panic!("{} is not in the list", l.native_name())))
            .collect();
        assert!(rows.windows(2).all(|w| w[0].y < w[1].y), "한국어, English, 中文, 日本語 from top to bottom: {rows:?}");
        let before = picked;
        let after = picker_click(&ctx, &mut picked, rows[Lang::ALL.iter().position(|l| *l == target).unwrap()]);
        assert_eq!(picked, target, "a click on {} picks it (it was {before:?})", target.native_name());
        assert!(spot(&after, |n| n.label() == Some(target.native_name())).is_none(), "the list shuts after the pick");
    }
}
// The stage: lines of the header that are left empty give their room to the timer (as in 1.1.0, whose
// stage was a grid of rows `auto 1fr` with an empty subtitle hidden). Numbers are the design at 850 x 590.
fn stage_of(brand: bool, event: bool, subtitle: bool, top_logos: Option<f32>) -> scene::Stage {
    scene::layout(Rect::from_min_size(Pos2::ZERO, vec2(850.0, 590.0)), 1.0, scene::Header { brand, event, subtitle, top_logos })
}
#[test]
fn stage_with_every_line_keeps_its_design() {
    let full = stage_of(true, true, true, None);
    assert_eq!((full.brand, full.logos, full.event, full.subtitle), (Some(42.0), None, Some(118.0), Some(152.0)));
    assert!((full.area.top() - 173.8).abs() < 1e-3 && full.area.bottom() == 546.0);
    assert!((full.area.center().y - 0.61 * 590.0).abs() < 1e-3, "the timer is centred at 61 % of the height, as it always was");
    assert!((full.timer - 1.0).abs() < 1e-5, "with every line shown the timer has the stage's own scale");
    // A logo row fits into the top band without moving anything.
    let logo = stage_of(true, true, true, Some(38.0));
    assert_eq!(logo.logos, Some(57.0));
    assert_eq!((logo.event, logo.subtitle, logo.area.top(), logo.timer), (full.event, full.subtitle, full.area.top(), full.timer));
    // At another size and place everything scales together.
    let big = scene::layout(
        Rect::from_min_size(pos2(10.0, 20.0), vec2(1700.0, 1180.0)),
        2.0,
        scene::Header { brand: true, event: true, subtitle: true, top_logos: None },
    );
    assert_eq!(big.event, Some(20.0 + 236.0));
    assert!((big.timer - 2.0).abs() < 1e-5);
}
#[test]
fn empty_lines_give_their_room_to_the_timer() {
    let full = stage_of(true, true, true, None);
    let no_subtitle = stage_of(true, true, false, None);
    let no_event = stage_of(true, false, true, None);
    let brand_only = stage_of(true, false, false, None);
    let nothing = stage_of(false, false, false, None);
    // No subtitle: the event name stays where it is and the timer's room starts higher.
    assert_eq!(no_subtitle.event, full.event);
    assert_eq!(no_subtitle.subtitle, None);
    assert!(no_subtitle.area.top() < full.area.top() && no_subtitle.timer > full.timer);
    // No event name: the subtitle moves up into its place, and the timer gains more than without a subtitle.
    assert_eq!(no_event.event, None);
    assert!(no_event.subtitle.unwrap() < full.event.unwrap());
    assert!(no_event.timer > no_subtitle.timer);
    // Neither: the room starts under the brand name. Nothing at all: at the margin, with the largest timer.
    assert!(brand_only.timer > no_event.timer);
    assert!(nothing.timer > brand_only.timer);
    assert_eq!(nothing.area.top(), 32.0);
    // The ring (radius 138.65 and half its 10 px line, at the timer's scale) stays clear of the header and of the status line.
    for s in [&full, &no_subtitle, &no_event, &brand_only, &nothing] {
        let header = [s.brand.map(|y| y + 10.0), s.event.map(|y| y + 18.0), s.subtitle.map(|y| y + 9.0)].into_iter().flatten().fold(0.0, f32::max);
        let ring = 143.65 * s.timer;
        assert!(s.area.center().y - ring > header, "{s:?}");
        assert!(s.area.center().y + ring < s.area.bottom(), "{s:?}");
        assert_eq!(s.area.bottom(), 546.0);
    }
}
#[test]
fn stage_without_a_brand_starts_at_the_margin() {
    // No brand, no event name, a subtitle and a logo at the top: logo at the margin, subtitle under it, the rest for the timer.
    let s = stage_of(false, false, true, Some(38.0));
    assert_eq!((s.logos, s.subtitle), (Some(32.0), Some(84.0)));
    assert!((s.area.top() - 105.8).abs() < 1e-3 && (s.timer - 440.2 / 372.2).abs() < 1e-4);
    // A tall logo pushes the titles down instead of running into them, and the timer makes room.
    let tall = stage_of(true, true, true, Some(114.0));
    assert_eq!((tall.logos, tall.event), (Some(57.0), Some(57.0 + 114.0 + 5.0 + 18.0)));
    assert!(tall.timer < 1.0);
    // On a narrow stage the ring is never wider than 52 % of it; on a very short one the timer does not vanish.
    let narrow = scene::layout(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 900.0)), 300.0 / 850.0, scene::Header::default());
    assert!((narrow.timer * 138.65 * 2.0 - 300.0 * 0.52).abs() < 0.01);
    let short = scene::layout(
        Rect::from_min_size(Pos2::ZERO, vec2(850.0, 60.0)),
        0.35,
        scene::Header { brand: true, event: true, subtitle: true, top_logos: Some(114.0) },
    );
    assert!(short.timer >= 0.35 * 0.25 && short.area.height() >= 0.0);
}
#[test]
fn the_drawn_timer_grows_as_the_header_empties() {
    let ctx = Context::default();
    install_fonts(&ctx, Lang::Ko, false);
    let draw = |s: &Settings| {
        let mut out = Rect::NOTHING;
        let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(850.0, 590.0))), ..Default::default() }, |ctx| {
            CentralPanel::default().frame(Frame::new()).show(ctx, |ui| {
                let v = Clock::new(300.0).view(0.0, 60.0, false, true, false);
                out = scene::render(ui.painter(), ui.max_rect(), s, &v, &Pictures::default(), false, false);
            });
        });
        out
    };
    for mode in [Mode::Circle, Mode::Bar, Mode::Number] {
        let mut s = Settings::default();
        s.mode = mode;
        assert!(!s.brand.is_empty() && !s.event.is_empty() && !s.subtitle.is_empty());
        let full = draw(&s);
        s.event.clear();
        let no_event = draw(&s);
        s.subtitle.clear();
        let no_titles = draw(&s);
        s.brand.clear();
        let bare = draw(&s);
        assert!(
            no_event.height() > full.height() && no_titles.height() > no_event.height() && bare.height() > no_titles.height(),
            "{mode:?}: {full:?} {no_event:?} {no_titles:?} {bare:?}"
        );
        assert!(bare.top() >= 32.0 && bare.bottom() <= 546.0, "{mode:?}: {bare:?}");
        s.event = "   ".into();
        s.subtitle = " ".into();
        assert_eq!(draw(&s), bare, "a line of spaces counts as empty");
    }
}
// The ring's middle is put on a whole pixel (here at 1.5 pixels per point), so its edge is drawn alike on every side.
#[test]
fn a_ring_is_centred_on_a_whole_pixel() {
    let ctx = Context::default();
    ctx.set_pixels_per_point(1.5);
    let mut got = Vec::new();
    for _ in 0..2 {
        let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 200.0))), ..Default::default() }, |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                got = [pos2(10.2, 7.9), pos2(469.67, 477.55), pos2(8.0, 4.0)].map(|at| on_pixels(ui.painter(), at)).to_vec();
            });
        });
    }
    for (at, want) in got.iter().zip([pos2(10.0, 8.0), pos2(470.0, 477.33334), pos2(8.0, 4.0)]) {
        assert!((at.x - want.x).abs() < 1e-3 && (at.y - want.y).abs() < 1e-3, "{at:?} {want:?}");
        assert!(
            ((at.x * 1.5).round() - at.x * 1.5).abs() < 1e-3 && ((at.y * 1.5).round() - at.y * 1.5).abs() < 1e-3,
            "{at:?} is not on a whole pixel"
        );
    }
}
// 자동: on the dark stage a logo that would get lost is shown white (1.1.0 showed the white version of its logo on a
// dark background and the colour version on a light one); logos that stand out, and every logo on a light background, keep their colours.
#[test]
fn auto_tone_whitens_only_logos_lost_on_a_dark_background() {
    let stage = scene::behind(&Settings::default(), &Pictures::default());
    let dark_green = [1.0 / 255.0, 26.0 / 255.0, 10.0 / 255.0];
    assert!(needs_white(dark_green, stage), "dark green");
    assert!(needs_white([0.1, 0.17, 0.37], stage), "navy");
    assert!(needs_white([0.27, 0.27, 0.27], stage), "dark grey");
    assert!(!needs_white([0.82, 0.01, 0.11], stage), "red stands out enough");
    assert!(!needs_white([0.0, 0.44, 0.94], stage), "blue stands out");
    assert!(!needs_white([0.96, 0.65, 0.14], stage), "orange stands out");
    assert!(!needs_white(dark_green, [0.92, 0.92, 0.92]), "on a light background the colours stay");
    assert!(!needs_white(dark_green, [0.55, 0.55, 0.55]), "mid grey is not a dark background");
}
// The white copy is the picture with every pixel white and its transparency kept; the mean colour leaves out what is transparent.
#[test]
fn white_copy_keeps_transparency_and_mean_colour_skips_it() {
    let im = image::RgbaImage::from_raw(2, 2, vec![1, 26, 10, 255, 1, 26, 10, 128, 200, 0, 0, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(white_copy(&im), vec![255, 255, 255, 255, 255, 255, 255, 128, 255, 255, 255, 0, 255, 255, 255, 0]);
    let (mean, opaque) = mean_color(&im);
    assert!((mean[0] - 1.0 / 255.0).abs() < 1e-6 && (mean[1] - 26.0 / 255.0).abs() < 1e-6 && (mean[2] - 10.0 / 255.0).abs() < 1e-6, "{mean:?}");
    assert!((opaque - (255.0 + 128.0) / 255.0 / 4.0).abs() < 1e-6, "{opaque}");
    assert_eq!(mean_color(&image::RgbaImage::new(3, 3)), ([0.0; 3], 0.0), "nothing opaque");
    // A colour transform keeps a channel at multiplier 256 and offset 0, and stays within 0..255.
    for c in [0u8, 1, 128, 254, 255] {
        assert_eq!(transform_channel(c, 256, 0), c);
        assert_eq!(transform_channel(c, 0, 255), 255);
    }
    assert_eq!((transform_channel(200, 512, 0), transform_channel(10, 256, -20), transform_channel(100, 128, 10)), (255, 0, 60));
}
// What the logos sit on: without a picture the stage's dark gradient, with one its mean colour darkened by 어둡게.
#[test]
fn behind_the_logos_is_the_dimmed_background_or_the_gradient() {
    let ctx = Context::default();
    let mut pics = Pictures::default();
    let mut s = Settings::default();
    let plain = scene::behind(&s, &pics);
    assert!(luminance(plain) < 0.02, "the stage without a picture is dark: {plain:?}");
    s.background = "assets/background.png".into();
    s.dim = 0.5;
    assert_eq!(scene::behind(&s, &pics), plain, "a picture that didn't load changes nothing");
    pics.own.insert(s.background.clone(), ctx.load_texture("bg", ColorImage::new([1, 1], Color32::WHITE), TextureOptions::LINEAR));
    pics.mean.insert(s.background.clone(), ([1.0, 1.0, 1.0], 1.0));
    assert_eq!(scene::behind(&s, &pics), [0.5; 3]);
    s.dim = 0.0;
    assert_eq!(scene::behind(&s, &pics), [1.0; 3]);
    pics.mean.insert(s.background.clone(), ([1.0, 1.0, 1.0], 0.5));
    let half = scene::behind(&s, &pics);
    assert!((half[0] - (1.0 + 18.0 / 255.0) / 2.0).abs() < 1e-6, "half transparent: over the base {half:?}");
}
// A dark green logo at the top right and only a subtitle: 자동 draws the white copy on the stage and the
// logo's own colours on the floating timer, over the slides; 원래 색 and 흰색 do what they say, 흰색 with a halo when floating.
#[test]
fn the_stage_draws_a_dark_logo_white() {
    let ctx = Context::default();
    install_fonts(&ctx, Lang::Ko, false);
    let im = image::RgbaImage::from_pixel(40, 12, image::Rgba([1, 26, 10, 255]));
    let name = "assets/brand-logo.png".to_string();
    let mut pics = Pictures::default();
    pics.own.insert(name.clone(), ctx.load_texture("own", ColorImage::from_rgba_unmultiplied([40, 12], im.as_raw()), TextureOptions::LINEAR));
    pics.white
        .insert(name.clone(), ctx.load_texture("white", ColorImage::from_rgba_unmultiplied([40, 12], &white_copy(&im)), TextureOptions::LINEAR));
    pics.mean.insert(name.clone(), mean_color(&im));
    let (own, white) = (pics.own[&name].id(), pics.white[&name].id());
    let mut s = Settings::default();
    s.logo = name.clone();
    s.brand.clear();
    s.event.clear();
    s.subtitle = "세션".into();
    let drawn = |s: &Settings, floating: bool| {
        let out = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(850.0, 590.0))), ..Default::default() }, |ctx| {
            CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
                let v = Clock::new(300.0).view(0.0, 60.0, false, true, false);
                if floating {
                    let _ = overlay::paint(ui.painter(), ui.max_rect(), s, &v, "", &pics);
                } else {
                    let _ = scene::render(ui.painter(), ui.max_rect(), s, &v, &pics, false, false);
                }
            });
        });
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                Shape::Mesh(m) => Some(m.texture_id),
                _ => None,
            })
            .filter(|id| *id == own || *id == white)
            .collect::<Vec<_>>()
    };
    assert_eq!(drawn(&s, false), [white], "자동 on the stage");
    s.overlay_scene = true;
    assert_eq!(drawn(&s, true), [own], "자동 on the floating timer keeps the colours");
    s.logo_tone = Tone::Own;
    assert_eq!(drawn(&s, false), [own], "원래 색");
    s.logo_tone = Tone::White;
    assert_eq!(drawn(&s, false), [white], "흰색");
    let floating = drawn(&s, true);
    assert_eq!(floating.len(), 17, "흰색 when floating: 16 halo copies and the logo");
    assert!(floating.iter().all(|id| *id == white));
}
// Saved settings from before have no logo colour: they start with 자동, and a choice is kept.
#[test]
fn saved_settings_without_a_logo_colour_start_with_auto() {
    let s: Settings = serde_json::from_str(r#"{"logo":"assets/brand-logo.png"}"#).unwrap();
    assert_eq!(s.logo_tone, Tone::Auto);
    let mut w = s.clone();
    w.logo_tone = Tone::White;
    let back: Settings = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
    assert_eq!(back.logo_tone, Tone::White);
}
// Output screens and shortcuts, driven by made-up key events (never the real keyboard).
fn key_frame(ctx: &Context, app: &mut Studio, keys: &[shortcuts::Binding], from: Option<usize>) {
    // Each key goes down and up again, as on a keyboard.
    let events = [true, false]
        .into_iter()
        .flat_map(|pressed| {
            keys.iter().map(move |b| Event::Key {
                key: b.key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers { alt: b.alt, ctrl: b.ctrl, shift: b.shift, mac_cmd: b.cmd, command: b.ctrl || b.cmd },
            })
        })
        .collect();
    let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), events, ..Default::default() }, |ctx| {
        app.shortcut_keys(ctx, from)
    });
}
fn default_key(a: Action) -> shortcuts::Binding {
    a.default_binding().expect("the action has a default key")
}
#[test]
fn a_shortcut_pressed_in_any_window_drives_the_one_clock() {
    let ctx = Context::default();
    let mut app = Studio::bare(Settings::default(), fixture("shortcut-test"), None);
    assert!(app.open_screen(Format::Number) && app.open_screen(Format::Presenter));
    // The editor and each output screen act on the same clock: what one starts, the others show.
    key_frame(&ctx, &mut app, &[default_key(Action::ToggleTimer)], None);
    assert!(app.clock.running() && app.facts().running);
    key_frame(&ctx, &mut app, &[default_key(Action::ToggleTimer)], Some(1));
    assert!(!app.clock.running());
    // Before the timer starts, the time is the setting: +1 minute and -10 seconds move both.
    key_frame(&ctx, &mut app, &[default_key(Action::Reset)], Some(1));
    let before = app.clock.duration;
    key_frame(&ctx, &mut app, &[default_key(Action::Add1m)], Some(0));
    key_frame(&ctx, &mut app, &[default_key(Action::Sub10s)], None);
    assert_eq!((app.clock.duration, app.s.duration), (before + 50.0, before + 50.0));
    // The display mode is one setting for the whole program.
    key_frame(&ctx, &mut app, &[default_key(Action::ModeBar)], Some(1));
    assert_eq!(app.s.mode, Mode::Bar);
    key_frame(&ctx, &mut app, &[default_key(Action::ModeNumber)], None);
    assert_eq!(app.s.mode, Mode::Number);
    // The editor asks before it resets a timer that has run; an output screen does not.
    key_frame(&ctx, &mut app, &[default_key(Action::ToggleTimer)], None);
    app.clock.accumulated = 30.0;
    key_frame(&ctx, &mut app, &[default_key(Action::Reset)], None);
    assert!(app.confirm_reset && app.clock.running());
    key_frame(&ctx, &mut app, &[default_key(Action::Reset)], Some(0));
    assert!(!app.confirm_reset && !app.clock.running() && app.clock.elapsed(app.now()) == 0.0);
}
#[test]
fn shortcuts_open_and_close_screens_and_stop_at_the_limit() {
    let ctx = Context::default();
    let mut app = Studio::bare(Settings::default(), fixture("shortcut-test"), None);
    app.new_format = Format::Mini;
    for n in 1..=MAX_SCREENS {
        key_frame(&ctx, &mut app, &[default_key(Action::OpenScreen)], None);
        assert_eq!(app.screens.len(), n);
    }
    assert!(app.screens.iter().all(|s| s.format == Format::Mini));
    assert_eq!(app.screens.iter().map(|s| s.id).collect::<Vec<_>>(), (1..=MAX_SCREENS as u32).collect::<Vec<_>>());
    key_frame(&ctx, &mut app, &[default_key(Action::OpenScreen)], None);
    assert_eq!(app.screens.len(), MAX_SCREENS, "no more than the limit");
    assert_eq!(app.status, tf!("출력 화면은 {0}개까지 열 수 있습니다", MAX_SCREENS));
    // A closed screen's number is given to the next one that opens.
    app.screens[2].closed = true;
    app.screens.retain(|s| !s.closed);
    assert!(app.open_screen(Format::Bar));
    assert_eq!(app.screens.last().map(|s| (s.id, s.format)), Some((3, Format::Bar)));
    // Close all: every screen is marked, and the windows go at the next frame. Closing has no default key.
    assert_eq!(Action::CloseScreens.default_binding(), None);
    app.run_action(Action::CloseScreens, &ctx, None);
    assert!(app.screens.iter().all(|s| s.closed));
    ctx.set_embed_viewports(true);
    let _ = ctx.run(RawInput::default(), |ctx| app.screen_windows(ctx));
    assert!(app.screens.is_empty());
}
#[test]
fn a_new_screen_shows_the_current_state_at_once() {
    let ctx = Context::default();
    ctx.set_embed_viewports(true);
    install_fonts(&ctx, Lang::Ko, false);
    let mut app = Studio::bare(Settings::default(), fixture("shortcut-test"), None);
    app.s.mode = Mode::Bar;
    app.clock.toggle(app.now());
    app.clock.adjust(-60.0, app.now());
    // Opened late, in the first frame it is drawn the screen already has what the editor has: no copying, no waiting.
    assert!(app.open_screen(Format::Follow));
    let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), ..Default::default() }, |ctx| {
        app.screen_windows(ctx)
    });
    let v = app.clock.view(app.now(), app.s.warning, app.s.count_up, app.s.overtime, app.s.seconds_only);
    assert_eq!(app.screens[0].format.look(app.s.mode), screens::Look::Stage(Mode::Bar));
    assert!(app.facts().running && app.facts().started && v.phase != Phase::Ready);
    assert_eq!(app.clock.duration, app.s.duration - 60.0);
}
#[test]
fn shortcuts_can_be_switched_off_and_do_not_fire_while_typing() {
    let ctx = Context::default();
    let mut app = Studio::bare(Settings::default(), fixture("shortcut-test"), None);
    let space = default_key(Action::ToggleTimer);
    app.s.shortcuts.enabled = false;
    key_frame(&ctx, &mut app, &[space], None);
    assert!(!app.clock.running(), "all off");
    app.s.shortcuts.enabled = true;
    // A text box has the keyboard: the key is the text box's.
    app.typing = true;
    key_frame(&ctx, &mut app, &[space, default_key(Action::Add1m)], None);
    assert!(!app.clock.running() && app.clock.duration == app.s.duration);
    // An output screen has no text box, so what the editor was typing does not matter there.
    assert!(app.open_screen(Format::Number));
    key_frame(&ctx, &mut app, &[space], Some(0));
    assert!(app.clock.running());
    // A function key types nothing: it works even while typing.
    let f6 = shortcuts::Binding::new(Key::F6);
    assert_eq!(app.s.shortcuts.assign(Action::Reset, f6), Assign::Set);
    key_frame(&ctx, &mut app, &[f6], None);
    assert!(app.confirm_reset || !app.clock.running());
}
#[test]
fn a_changed_key_works_and_the_old_one_does_not_and_the_hints_follow() {
    let ctx = Context::default();
    let mut app = Studio::bare(Settings::default(), fixture("shortcut-test"), None);
    let combo = shortcuts::Binding { ctrl: true, shift: true, ..shortcuts::Binding::new(Key::T) };
    assert_eq!(app.s.shortcuts.assign(Action::ToggleTimer, combo), Assign::Set);
    key_frame(&ctx, &mut app, &[default_key(Action::ToggleTimer)], None);
    assert!(!app.clock.running(), "the old key is free");
    key_frame(&ctx, &mut app, &[combo], None);
    assert!(app.clock.running(), "the new key starts it");
    // Without Shift it is another key: nothing happens.
    key_frame(&ctx, &mut app, &[shortcuts::Binding { ctrl: true, ..shortcuts::Binding::new(Key::T) }], None);
    assert!(app.clock.running());
    // The hints on the buttons and in the tips show the key that is set, or none when they are off.
    app.refresh_hints();
    assert_eq!((app.hints.toggle.as_str(), app.hints.reset.as_str()), ("Ctrl+Shift+T", "R"));
    app.s.shortcuts.enabled = false;
    app.refresh_hints();
    assert_eq!((app.hints.toggle.as_str(), app.hints.reset.as_str()), ("", ""));
    // Giving a key that another action has takes it from that action.
    app.s.shortcuts.enabled = true;
    assert_eq!(app.s.shortcuts.assign(Action::Add1m, shortcuts::Binding::new(Key::R)), Assign::Replaced(Action::Reset));
    assert_eq!(app.s.shortcuts.binding(Action::Reset), None);
}
#[test]
fn screens_and_shortcuts_are_saved_with_the_settings() {
    let mut s = Settings::default();
    s.screens = vec![Format::Bar, Format::Presenter, Format::Mini];
    s.restore_screens = true;
    s.shortcuts.assign(Action::Fullscreen, shortcuts::Binding::new(Key::F));
    s.shortcuts.clear(Action::OpenScreen);
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.screens, s.screens);
    assert!(back.restore_screens);
    assert_eq!(back.shortcuts.binding(Action::Fullscreen), Some(shortcuts::Binding::new(Key::F)));
    assert_eq!(back.shortcuts.binding(Action::OpenScreen), None);
    assert_eq!(back.shortcuts.binding(Action::Reset), Some(shortcuts::Binding::new(Key::R)));
    // Settings from before have no screens and all shortcuts on with their default keys.
    let old: Settings = serde_json::from_str(r#"{"event":"x"}"#).unwrap();
    assert!(old.screens.is_empty() && !old.restore_screens && old.shortcuts.enabled && old.shortcuts.custom.is_empty());
    // Too many saved screens are cut to the limit; an odd shortcut entry is dropped.
    let mut many: Settings = serde_json::from_str(&format!(
        r#"{{"screens":[{}],"shortcuts":{{"custom":{{"toggle":"Ctrl+Alt+Nope","nonsense":"R","reset":"Cmd+Q"}}}}}}"#,
        [r#""Bar""#; MAX_SCREENS + 4].join(",")
    ))
    .unwrap();
    many.normalize();
    assert_eq!(many.screens.len(), MAX_SCREENS);
    assert!(many.shortcuts.custom.is_empty(), "{:?}", many.shortcuts.custom);
}
