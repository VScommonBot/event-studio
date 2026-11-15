//! The setup and removal windows.
//!
//! Setup programs usually walk through welcome, folder, progress and finish pages, and a
//! removal asks one yes/no question. Copying one small program takes a moment, so here the
//! pages are one screen to choose and one to finish, with the words people know from setups
//! ("설치 폴더", "찾아보기...", "설치 완료", "마침").

use crate::lang::{self, t, tf, Lang};
use crate::pack::{self, Pack};
use crate::setup::{self, Carried, Found, Installed, Job, Options, Places, Removed};
use crate::studio::{self, BG, END, LINE, MUTED, PANEL, WHITE};
use eframe::egui::{self, *};
use std::path::PathBuf;
use std::time::Duration;

const SIZE: Vec2 = vec2(600.0, 570.0);
const PROBLEM_BACK: Color32 = Color32::from_rgb(48, 26, 32);
const ACCENT: Color32 = Color32::from_rgb(132, 226, 190);
/// `--screenshot`: the picture is taken after this many frames, like the timer's own.
const SHOT_FRAME: u32 = 120;
const SHOT_GIVE_UP: u32 = 300;

enum Page {
    Ready,
    /// Shown from this frame on, so the window says what it is doing before it does it.
    Working(u32),
    Installed(Installed),
    Removed(Removed),
    Problem(String),
}

struct SetupWindow {
    install: Option<(Pack, Places)>,
    remove: Option<(PathBuf, Places)>,
    page: Page,
    /// The language of the window, switched at its top right.
    language: Lang,
    dir: PathBuf,
    desktop: bool,
    start_menu: bool,
    replace_settings: bool,
    run_after: bool,
    remove_data: bool,
    /// Brand and event name in the settings the setup file carries.
    carried: Option<(String, String)>,
    /// One more font is loaded, for a carried brand or event name in a script this language's fonts lack.
    wide_fonts: bool,
    /// For checks (`--demo existing`): what to pretend the folder holds.
    pretend: Option<Found>,
    error: Option<String>,
    shot: Option<PathBuf>,
    frames: u32,
    shot_requested: bool,
    shot_saved: bool,
}

fn carried_names(pack: &Pack) -> Option<(String, String)> {
    let text = pack.read(pack::SETTINGS).ok()??;
    let fields: serde_json::Value = serde_json::from_slice(&text).ok()?;
    let field = |name: &str| fields.get(name).and_then(|v| v.as_str()).unwrap_or_default().trim().to_string();
    Some((field("brand"), field("event")))
}

impl SetupWindow {
    fn new(job: Job, shot: Option<PathBuf>, demo: Option<String>) -> Self {
        let mut w = SetupWindow {
            install: None,
            remove: None,
            page: Page::Ready,
            language: lang::current(),
            dir: PathBuf::new(),
            desktop: true,
            start_menu: true,
            replace_settings: false,
            run_after: true,
            remove_data: false,
            carried: None,
            wide_fonts: false,
            pretend: None,
            error: None,
            shot,
            frames: 0,
            shot_requested: false,
            shot_saved: false,
        };
        match job {
            Job::Install { pack, places, language } => {
                w.language = language;
                w.dir = places.default_dir();
                w.carried = carried_names(&pack);
                w.install = Some((pack, places));
            }
            Job::Uninstall { dir, places, language } => {
                w.language = language;
                w.dir = dir.clone();
                w.remove = Some((dir, places));
            }
            Job::Problem(text, language) => (w.page, w.language) = (Page::Problem(text), language),
        }
        lang::set(w.language);
        // `--demo a,b`: fixed states for checks.
        for d in demo.as_deref().unwrap_or_default().split(',') {
            match d {
                "done" => w.page = Page::Installed(Installed { dir: w.dir.clone(), exe: w.dir.join(setup::EXE_NAME), settings: Carried::Written }),
                "removed" => w.page = Page::Removed(Removed { dir: w.dir.clone(), data_kept: true, left: Vec::new() }),
                "existing" => w.pretend = Some(Found { program: true, settings: true }),
                "error" => w.error = Some(t(setup::RUNNING).into()),
                _ => {}
            }
        }
        w
    }

    fn removing(&self) -> bool {
        self.remove.is_some()
    }

    fn found(&self) -> Found {
        self.pretend.unwrap_or_else(|| setup::look(&self.dir))
    }

    /// The step the window is waiting to do.
    fn work(&mut self) {
        if let Some((pack, places)) = &self.install {
            let o = Options { dir: self.dir.clone(), desktop: self.desktop, start_menu: self.start_menu, replace_settings: self.replace_settings, language: self.language };
            match setup::install(pack, places, &o) {
                Ok(done) => self.page = Page::Installed(done),
                Err(e) => (self.page, self.error) = (Page::Ready, Some(e)),
            }
        } else if let Some((dir, places)) = &self.remove {
            match setup::uninstall(dir, places, self.remove_data, setup::WINDOW_PATIENCE) {
                Ok(done) => self.page = Page::Removed(done),
                Err(e) => (self.page, self.error) = (Page::Ready, Some(e)),
            }
        }
    }

    fn title(&self) -> &'static str {
        if self.removing() { t("Event Studio 제거") } else { t("Event Studio 설치") }
    }

    /// Switches the window to another language: its texts, its fonts and its title.
    fn set_language(&mut self, ctx: &Context, language: Lang) {
        self.language = language;
        lang::set(language);
        studio::install_fonts(ctx, language, self.wide_fonts);
        ctx.send_viewport_cmd(ViewportCommand::Title(self.title().into()));
        // A problem that is being shown was written in the old language.
        self.error = None;
    }

    fn header(&mut self, ctx: &Context) {
        let mut picked = self.language;
        TopBottomPanel::top("header").frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(24, 17))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(3.0_f32, ACCENT));
                ui.painter().line_segment([r.center(), r.center() + vec2(0.0, -6.0)], Stroke::new(2.0_f32, ACCENT));
                ui.label(RichText::new("EVENT STUDIO").size(19.0).strong());
                ui.add_space(12.0);
                ui.label(RichText::new(if self.removing() { t("제거\u{1}uninstall") } else { t("설치") }).color(MUTED).size(13.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    studio::language_picker(ui, &mut picked);
                    ui.label(RichText::new(tf!("버전 {0}", setup::VERSION)).color(MUTED).size(11.0));
                });
            });
        });
        if picked != self.language {
            self.set_language(ctx, picked);
        }
    }

    fn primary(ui: &mut Ui, text: &str, enabled: bool) -> bool {
        let button = Button::new(RichText::new(text).color(BG).strong()).fill(ACCENT).corner_radius(9);
        ui.add_enabled(enabled, button.min_size(vec2(118.0, 40.0))).clicked()
    }

    fn footer(&mut self, ctx: &Context) {
        TopBottomPanel::bottom("buttons").frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(24, 16))).show(ctx, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let close = || ctx.send_viewport_cmd(ViewportCommand::Close);
                match &self.page {
                    Page::Ready => {
                        let label = if self.removing() { t("제거\u{1}uninstall") } else { t("설치") };
                        if Self::primary(ui, label, true) {
                            (self.error, self.page) = (None, Page::Working(self.frames));
                        }
                        if ui.add(Button::new(t("취소")).min_size(vec2(84.0, 40.0))).clicked() {
                            close();
                        }
                    }
                    Page::Working(_) => {
                        Self::primary(ui, t("잠시만요…"), false);
                    }
                    Page::Installed(done) => {
                        if Self::primary(ui, t("마침"), true) {
                            if self.run_after {
                                let _ = std::process::Command::new(&done.exe).current_dir(&done.dir).spawn();
                            }
                            close();
                        }
                    }
                    Page::Removed(_) | Page::Problem(_) => {
                        if Self::primary(ui, t("닫기"), true) {
                            close();
                        }
                    }
                }
            });
        });
    }

    fn note(ui: &mut Ui, text: impl Into<String>) {
        ui.label(RichText::new(text).color(MUTED).size(12.0));
    }

    fn folder_box(ui: &mut Ui, dir: &std::path::Path, browse: Option<&mut bool>) {
        Frame::new().fill(BG).stroke(Stroke::new(1.0_f32, LINE)).corner_radius(8).inner_margin(10).show(ui, |ui| {
            ui.horizontal(|ui| {
                // A long folder runs over two lines instead of being cut off: its end, the program's own folder, matters most.
                let room = (ui.available_width() - if browse.is_some() { 112.0 } else { 0.0 }).max(120.0);
                ui.allocate_ui_with_layout(vec2(room, 0.0), Layout::top_down(Align::LEFT), |ui| {
                    ui.set_width(room);
                    ui.add(Label::new(RichText::new(dir.display().to_string()).size(12.5)).wrap());
                });
                if let Some(pressed) = browse {
                    *pressed = ui.button(t("찾아보기...")).clicked();
                }
            });
        });
    }

    fn install_page(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(t("Event Studio를 이 컴퓨터에 설치합니다")).size(20.0).strong());
        Self::note(ui, t("브랜드 행사 타이머 · 관리자 권한 없이, 지금 로그인한 계정에 설치됩니다."));
        if let Some((brand, event)) = &self.carried {
            ui.add_space(6.0);
            let names: Vec<&str> = [brand.as_str(), event.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
            ui.label(RichText::new(t("이 설치 파일에는 브랜드 · 행사 설정과 이미지가 담겨 있습니다")).color(ACCENT).size(12.5));
            if !names.is_empty() {
                Self::note(ui, names.join("  ·  "));
            }
        }
        ui.add_space(6.0);
        Self::note(ui, t("설치 폴더"));
        let mut browse = false;
        Self::folder_box(ui, &self.dir, Some(&mut browse));
        if browse {
            let from = self.dir.ancestors().find(|p| p.is_dir()).map(PathBuf::from).unwrap_or_default();
            if let Some(picked) = rfd::FileDialog::new().set_title(t("설치할 폴더 선택")).set_directory(from).pick_folder() {
                self.dir = setup::with_app_folder(&picked);
                self.error = None;
            }
        }
        let found = self.found();
        if found.program {
            Self::note(ui, t("이 폴더에 설치된 Event Studio를 새 것으로 바꿉니다."));
        }
        if found.settings {
            if self.carried.is_some() {
                ui.checkbox(&mut self.replace_settings, t("설치 파일에 담긴 설정으로 바꾸기"));
                Self::note(ui, if self.replace_settings { t("지금 설정과 이미지는 설치 폴더의 previous-settings 폴더에 보관합니다.") } else { t("끄면 이 컴퓨터에 저장된 설정과 이미지를 그대로 씁니다.") });
            } else {
                Self::note(ui, t("이 컴퓨터에 저장된 설정과 이미지는 그대로 둡니다."));
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.desktop, t("바탕 화면에 바로 가기 만들기"));
            ui.add_space(14.0);
            ui.checkbox(&mut self.start_menu, t("시작 메뉴에 등록"));
        });
    }

    fn installed_page(&mut self, ui: &mut Ui, done: &Installed) {
        ui.label(RichText::new(t("설치 완료")).size(20.0).strong().color(ACCENT));
        ui.label(t("Event Studio의 설치가 완료되었습니다."));
        ui.add_space(6.0);
        Self::folder_box(ui, &done.dir, None);
        ui.add_space(4.0);
        let opened_from = match (self.desktop, self.start_menu) {
            (true, true) => t("바탕 화면이나 시작 메뉴의 'Event Studio'로 엽니다."),
            (true, false) => t("바탕 화면의 'Event Studio'로 엽니다."),
            (false, true) => t("시작 메뉴의 'Event Studio'로 엽니다."),
            (false, false) => t("설치 폴더의 EventStudio로 엽니다."),
        };
        Self::note(ui, format!("·  {opened_from}"));
        let settings = match done.settings {
            Carried::Written => t("·  설치 파일에 담긴 브랜드 · 행사 설정과 이미지를 넣었습니다."),
            Carried::KeptExisting => t("·  이 컴퓨터에 저장돼 있던 설정과 이미지를 그대로 씁니다."),
            Carried::Replaced => t("·  설치 파일의 설정으로 바꿨습니다. 이전 설정은 설치 폴더의 previous-settings 폴더에 있습니다."),
            Carried::Nothing => "",
        };
        if !settings.is_empty() {
            Self::note(ui, settings);
        }
        Self::note(ui, t("·  지울 때는 Windows 설정 > 앱 > 설치된 앱에서 'Event Studio'를 제거하세요."));
        ui.add_space(10.0);
        ui.checkbox(&mut self.run_after, t("Event Studio 실행하기"));
    }

    fn remove_page(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(t("Event Studio를 제거할까요?")).size(20.0).strong());
        Self::note(ui, t("프로그램과 바로 가기, 설치된 앱 목록의 항목을 지웁니다."));
        ui.add_space(10.0);
        Self::note(ui, t("설치 폴더"));
        Self::folder_box(ui, &self.dir, None);
        ui.add_space(8.0);
        ui.checkbox(&mut self.remove_data, t("내 설정과 이미지도 함께 지우기"));
        Self::note(ui, if self.remove_data { t("브랜드 · 행사 설정, 넣어 둔 로고 · 배경 · 알림음이 지워집니다. 되돌릴 수 없습니다.") } else { t("지우지 않으면 설치 폴더에 남아, 다시 설치했을 때 그대로 쓸 수 있습니다.") });
    }

    fn removed_page(ui: &mut Ui, done: &Removed) {
        ui.label(RichText::new(t("제거 마침")).size(20.0).strong().color(ACCENT));
        ui.label(t("Event Studio는 완전히 제거되었습니다."));
        if done.data_kept {
            ui.add_space(6.0);
            Self::note(ui, t("설정과 이미지는 다음 폴더에 남겨 두었습니다."));
            Self::folder_box(ui, &done.dir, None);
        }
        if !done.left.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(t("다음 파일은 지우지 못했습니다. 직접 지워 주세요.")).color(END).size(12.5));
            for file in &done.left {
                Self::note(ui, file.display().to_string());
            }
        }
    }

    fn body(&mut self, ctx: &Context) {
        CentralPanel::default().frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(28, 18))).show(ctx, |ui| {
            // A little tighter than the editor; the page scrolls if a small screen leaves it short.
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.spacing_mut().interact_size.y = 26.0;
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // The pages borrow their outcome while they draw, so it is taken out for the moment.
            match std::mem::replace(&mut self.page, Page::Ready) {
                Page::Ready => {
                    if self.removing() {
                        self.remove_page(ui);
                    } else {
                        self.install_page(ui);
                    }
                }
                Page::Working(since) => {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        ui.add(Spinner::new().size(28.0).color(ACCENT));
                        ui.add_space(8.0);
                        ui.label(if self.removing() { t("제거중") } else { t("설치중") });
                        Self::note(ui, t("잠시 기다려 주세요."));
                    });
                    self.page = Page::Working(since);
                }
                Page::Installed(done) => {
                    self.installed_page(ui, &done);
                    self.page = Page::Installed(done);
                }
                Page::Removed(done) => {
                    Self::removed_page(ui, &done);
                    self.page = Page::Removed(done);
                }
                Page::Problem(text) => {
                    ui.label(RichText::new(t("진행할 수 없습니다")).size(20.0).strong().color(END));
                    ui.label(RichText::new(&text).color(WHITE));
                    self.page = Page::Problem(text);
                }
            }
            });
        });
    }

    /// A problem is told in a strip of its own above the buttons, where it cannot scroll out of sight.
    fn problem_strip(&self, ctx: &Context) {
        let Some(e) = &self.error else { return };
        TopBottomPanel::bottom("problem").frame(Frame::new().fill(PROBLEM_BACK).inner_margin(Margin::symmetric(28, 10))).show(ctx, |ui| {
            ui.label(RichText::new(e).color(END).size(12.5));
        });
    }

    /// `--screenshot`: saves the window's picture after `SHOT_FRAME` frames and closes, doing no work.
    fn screenshot(&mut self, ctx: &Context) {
        let Some(path) = self.shot.clone() else { return };
        for event in ctx.input(|i| i.events.clone()) {
            if let Event::Screenshot { image, .. } = event {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
                match image::save_buffer(&path, &bytes, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8) {
                    Ok(()) => self.shot_saved = true,
                    Err(e) => eprintln!("Screenshot failed: {e}"),
                }
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        if !self.shot_requested && self.frames >= SHOT_FRAME {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
            self.shot_requested = true;
        }
        if self.frames > SHOT_GIVE_UP && !self.shot_saved {
            eprintln!("Screenshot timeout");
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

impl eframe::App for SetupWindow {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.frames += 1;
        if let (false, Some((brand, event))) = (self.wide_fonts, &self.carried) {
            if !ctx.fonts(|f| f.has_glyphs(&FontId::proportional(14.0), &format!("{brand}{event}"))) {
                self.wide_fonts = true;
                studio::install_fonts(ctx, self.language, true);
            }
        }
        self.screenshot(ctx);
        if let Page::Working(since) = self.page {
            // One frame with the "working" page on screen, then the work itself.
            if self.frames > since + 2 && self.shot.is_none() {
                self.work();
            }
            ctx.request_repaint();
        }
        self.header(ctx);
        self.footer(ctx);
        self.problem_strip(ctx);
        self.body(ctx);
    }
}

pub fn run(job: Job, shot: Option<PathBuf>, demo: Option<String>) -> eframe::Result {
    let language = match &job {
        Job::Install { language, .. } | Job::Uninstall { language, .. } | Job::Problem(_, language) => *language,
    };
    lang::set(language);
    let title = if matches!(job, Job::Uninstall { .. }) { t("Event Studio 제거") } else { t("Event Studio 설치") };
    let viewport = ViewportBuilder::default().with_title(title).with_inner_size(SIZE).with_resizable(false).with_maximize_button(false).with_icon(studio::window_icon());
    // A check's picture is taken without taking the keyboard from whoever is at the computer.
    let viewport = if shot.is_some() { viewport.with_active(false) } else { viewport };
    // `--demo hidden`: beyond the edge of the screen, so nothing flashes up in front of the person.
    let hidden = shot.is_some() && demo.as_deref().is_some_and(|d| d.split(',').any(|x| x == "hidden"));
    let viewport = if hidden { viewport.with_position(studio::OUT_OF_SIGHT) } else { viewport };
    let options = eframe::NativeOptions { viewport, centered: true, renderer: eframe::Renderer::Glow, ..Default::default() };
    eframe::run_native(
        "Event Studio Setup",
        options,
        Box::new(move |cc| {
            studio::theme(&cc.egui_ctx, language);
            Ok(Box::new(SetupWindow::new(job, shot, demo)))
        }),
    )
}
