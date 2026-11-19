#[path="scene.rs"] mod scene;
#[path="overlay.rs"] mod overlay;
#[path="controls.rs"] mod controls;
#[cfg(test)] #[path="studio_tests.rs"] mod tests;
use crate::timer::{self,Clock,Phase};
use crate::win;
use crate::setup;
use crate::lang::{self,t,tf,Lang};
use eframe::egui::{self,*};
use serde::{Deserialize,Serialize};
use std::{path::{Path,PathBuf},time::{Duration,Instant},collections::HashMap,sync::mpsc::Receiver};
pub(crate) const BG:Color32=Color32::from_rgb(15,19,25);
pub(crate) const PANEL:Color32=Color32::from_rgb(23,28,36);
pub(crate) const LINE:Color32=Color32::from_rgb(47,56,68);
pub(crate) const MUTED:Color32=Color32::from_rgb(146,158,174);
pub(crate) const WHITE:Color32=Color32::from_rgb(236,241,246);
const WARN:Color32=Color32::from_rgb(255,190,100);
pub(crate) const END:Color32=Color32::from_rgb(255,111,126);
const EDITOR_TITLE:&str="Event Studio · 브랜드 행사 타이머";
const FLOAT_TITLE:&str="행사 타이머";
const EDITOR_SIZE:Vec2=vec2(1280.0,860.0);
const EDITOR_MIN:Vec2=vec2(1000.0,720.0);
// The floating timer's sizes and timings are those of the overlay of VS Event Timer 1.1.0, this
// program's Electron predecessor; the file named behind a value is where 1.1.0 had it.
const FLOAT_SIZE:Vec2=vec2(360.0,200.0); // placement.js SIZES.overlay.def
const FLOAT_MIN:Vec2=vec2(160.0,80.0); // placement.js SIZES.overlay.min
const FLOAT_MARGIN:f32=24.0; // placement.js MARGIN
const CONTROLS_IDLE:f64=3.0; // app.js wake(): controls hide 3000 ms after the mouse stops
const CONTROLS_FADE:f32=0.3; // style.css #controls transition: opacity 0.3s
const CONTROL_BUTTON:f32=40.0; // style.css button min-height 40px
const CONTROL_BUTTON_FLOATING:f32=28.0; // style.css body.overlay #controls button 28px
const GRIP:f32=24.0; // style.css #resize-handle 24px
const TOPMOST_EVERY:f64=1.0; // the floating timer is put back in front every 1000 ms
const DRAG_PX:f32=5.0; // gesture.js DRAG_PX: a press that moves 5 px is a drag, not a click
const ZOOM_STEP:f32=1.1; // app.js wheel: Ctrl+wheel scales the window by 1.1
const FLASH_SECS:f64=6.0; // app.js onCross: the timer blinks for 6000 ms at the end
const BLINK_OPACITY:f32=0.4; // style.css blink-soft: 50% { opacity: 0.4 }
const BACKDROP_MAX:f32=0.8; // settings.js RANGES overlayBackdrop [0, 0.8]
/// A settings file that could not be read is kept under this name, so it never takes the backup's place.
pub(crate) const UNREADABLE_NAME:&str="settings.unreadable.json";
/// A changed setting is saved once it has stood this long.
const SAVE_AFTER:Duration=Duration::from_millis(600);
// How often the window is drawn again (`next_frame_in`).
const FRAME:Duration=Duration::from_nanos(16_666_667); // every frame, 60 times a second: only while something needs it
const FRAME_LATE:Duration=Duration::from_millis(2); // a frame asked for a moment falls just behind it
const SETTLE_FRAMES:u32=10; // every frame for this many after a window opens or changes its kind
const GRAPH_STEPS:f64=3000.0; // the ring or bar is drawn this often over the timer's length, about a pixel a step
const GRAPH_SLOWEST:f64=0.25; // but at least four times a second
const POINTER_POLL:f64=0.05; // macOS: where the pointer is over the floating timer is looked up this often (win_mac.rs)
/// The event name a new set of settings starts with; in another language it starts as that language's wording.
pub(crate) const FIRST_EVENT:&str="새로운 가능성을 여는 시간";
/// Where `--demo hidden` puts a check's window: beyond every screen.
pub(crate) const OUT_OF_SIGHT:Pos2=pos2(-12000.0,-12000.0);
/// Narrower than this, the top bar leaves its subtitle out so that the buttons of every language fit.
const HEADER_ROOMY:f32=1180.0;
fn phase_color(phase:Phase,accent:Color32)->Color32{match phase{Phase::Warning=>WARN,Phase::End|Phase::Over=>END,_=>accent}}
fn bold()->FontFamily{FontFamily::Name("bold".into())}
/// A ring whose line is centred on radius `r`, so an arc drawn at `r` lies exactly on it.
/// (`circle_stroke` puts the whole line outside the radius instead.)
fn ring(p:&Painter,c:Pos2,r:f32,width:f32,color:Color32){let n=180;let pts=(0..n).map(|i|{let a=std::f32::consts::TAU*i as f32/n as f32;c+vec2(a.cos(),a.sin())*r}).collect();p.add(Shape::closed_line(pts,Stroke::new(width,color)));}
/// `at` moved onto the nearest whole pixel: a ring drawn around it gets the same edge on every side.
fn on_pixels(p:&Painter,at:Pos2)->Pos2{let ppp=p.ctx().pixels_per_point();pos2((at.x*ppp).round()/ppp,(at.y*ppp).round()/ppp)}
#[derive(Clone,Serialize,Deserialize,PartialEq,Debug)] enum Mode{Circle,Bar,Number}
/// How the logos are coloured: 자동 (on the stage a logo that would get lost on its dark
/// background is shown white, the way VS Event Timer 1.1.0 showed the white version of its logo on
/// a dark background and the colour version on a light one), 원래 색 or 흰색.
#[derive(Clone,Copy,Serialize,Deserialize,PartialEq,Debug)] enum Tone{Auto,Own,White}
#[derive(Clone,Serialize,Deserialize)] #[serde(default)]
struct Settings{
 brand:String,event:String,subtitle:String,mode:Mode,duration:f64,warning:f64,
 qna_enabled:bool,qna_duration:f64,carry:bool,count_up:bool,overtime:bool,seconds_only:bool,
 background:String,logo:String,logo2:String,dim:f32,cover:bool,logo_position:usize,logo_size:f32,logo_tone:Tone,
 accent:[u8;3],sound:bool,volume:f32,sound_file:String,pin:bool,
 /// Floating timer: open the app as it, how dark its backdrop is (0 = see-through), show the
 /// brand, event name and logos in it too, where it was last (physical pixels) and whether
 /// that was the top-right corner.
 overlay_start:bool,overlay_backdrop:f32,overlay_scene:bool,overlay_rect:Option<[i32;4]>,overlay_top_right:bool,hotkeys:bool,
 /// Which build wrote the file (0: before the floating timer's background became always see-through).
 schema:u32,
 /// The interface's language (1.1.0 settings.js `language`: 'ko' by default; 'zh' and 'ja' are new).
 language:Lang,
}
const SCHEMA:u32=2;
impl Default for Settings{fn default()->Self{Self{
 brand:"YOUR BRAND".into(),event:FIRST_EVENT.into(),subtitle:format!("{}  ·  INSPIRATION SESSION",crate::report::this_year()),mode:Mode::Circle,duration:timer::DEFAULT_SECONDS,warning:timer::DEFAULT_WARNING,
 qna_enabled:false,qna_duration:timer::DEFAULT_SECONDS,carry:true,count_up:false,overtime:true,seconds_only:false,background:String::new(),logo:String::new(),logo2:String::new(),dim:0.55,cover:true,logo_position:2,logo_size:100.0,logo_tone:Tone::Auto,accent:[132,226,190],sound:true,volume:0.8,sound_file:String::new(),pin:false,
 overlay_start:false,overlay_backdrop:0.0,overlay_scene:false,overlay_rect:None,overlay_top_right:true,hotkeys:false,schema:0,language:Lang::Ko}}}
impl Settings{
 /// The settings of a very first start, when nothing is saved yet: in the language Windows is shown in
 /// (as setup programs pick theirs), with the event name's placeholder in that language.
 fn first(language:Lang)->Self{Self{language,event:lang::tr(language,FIRST_EVENT).into(),..Self::default()}}
 /// Brings a file from an older build up to date. Before schema 2 the floating timer painted the
 /// whole stage, background and all, when `overlay_scene` was on and ignored the backdrop: a
 /// backdrop saved then was never seen, so it starts again from see-through.
 fn migrate(&mut self){if self.schema<2&&self.overlay_scene{self.overlay_backdrop=0.0;}self.schema=SCHEMA;}
 fn normalize(&mut self){for v in [&mut self.duration,&mut self.qna_duration,&mut self.warning]{if !v.is_finite(){*v=300.0;}*v=v.clamp(0.0,timer::MAX_SECONDS).round();}self.dim=self.dim.clamp(0.0,1.0);self.volume=self.volume.clamp(0.0,1.0);self.logo_size=self.logo_size.clamp(20.0,300.0);self.logo_position=self.logo_position.min(5);
  self.overlay_backdrop=if self.overlay_backdrop.is_finite(){self.overlay_backdrop.clamp(0.0,BACKDROP_MAX)}else{0.0};if self.overlay_rect.is_some_and(|r|r[2]<1||r[3]<1){self.overlay_rect=None;}}
 fn accent(&self)->Color32{Color32::from_rgb(self.accent[0],self.accent[1],self.accent[2])}
}
#[derive(Clone,Copy,PartialEq,Debug)] enum GestureKind{Move,Resize}
/// A press on the floating timer: where the pointer and the window started (physical pixels).
#[derive(Clone,Copy,Debug)] struct Gesture{kind:GestureKind,cursor:(i32,i32),rect:win::PxRect,moved:bool}
struct Studio{
 s:Settings,clock:Clock,epoch:Instant,root:PathBuf,textures:Pictures,tab:usize,status:String,last_saved:String,dirty_since:Option<Instant>,presentation:bool,audience:bool,audience_full:bool,confirm_reset:bool,prior_phase:Phase,shot:Option<PathBuf>,frames:u32,shot_requested:bool,screenshot_ok:bool,
 /// The window is the floating timer (no background, always on top) instead of the editor.
 overlay:bool,hwnd:Option<isize>,editor_rect:Option<win::PxRect>,geometry:Option<(win::PxRect,u32)>,place_overlay:bool,last_topmost:f64,
 activity:Instant,audience_activity:Instant,bars:HashMap<&'static str,Rect>,float_buttons:Vec<(controls::Cmd,Rect)>,gesture:Option<Gesture>,flash_since:Option<Instant>,force_controls:bool,
 /// For checks (`--demo pulse`): the hover buttons are forced on between these seconds after the start.
 pulse:Option<(f64,f64)>,
 /// For checks (`--demo float`): the editor turns into the floating timer by itself at this frame,
 /// the way the 투명 타이머 실행 button does.
 auto_float:Option<u32>,
 /// For checks (`--demo back`): the floating timer goes back to the editor by itself at this frame,
 /// the way its gear button does.
 auto_back:Option<u32>,
 /// For checks (`--demo to-en`, `to-zh`, `to-ja`, `to-ko`): the language is switched at frame 40, the way the language box does it.
 auto_language:Option<Lang>,
 hotkeys:Option<(win::Hotkeys,Receiver<u8>)>,hotkey_failed:bool,
 /// Ctrl+1/2/3, live only while the floating timer is up.
 digit_hotkeys:Option<(win::Hotkeys,Receiver<u8>)>,digit_failed:bool,
 /// Clicks go through the floating timer's empty pixels: the window
 /// region last set, `None` for the whole window.
 hit_region:Option<Vec<win::HitShape>>,
 /// The 설치 파일 만들기 window, while it is open.
 packer:Option<Packer>,
 /// One more font is loaded for characters the language's own fonts lack; `font_text` is the text last looked at.
 wide_fonts:bool,font_text:String,
 /// For checks (`--demo panelend`): the settings panel starts scrolled to its end.
 panel_end:bool,
 /// For checks (`--demo`): their fixed states count frames, so every frame is drawn.
 steady:bool,
 /// Every frame is drawn until this one (`settle`).
 settle_until:u32,
 /// macOS: where the pointer was last found over the floating timer (`raw_input_hook`).
 pointer_at:Option<(i32,i32)>,
}
/// The 설치 파일 만들기 window: whether the settings go along, and how the last try ended.
struct Packer{with_settings:bool,made:Option<PathBuf>,error:Option<String>}
/// Where a macOS app keeps what it saves: an app is a folder that has to stay as it was built
/// (and often cannot be written), so for a program inside one (`….app/Contents/MacOS`) that is
/// the person's Application Support folder. `None` for a program on its own, which keeps its
/// settings beside itself as on Windows.
fn bundle_data_folder(dir:&Path,home:Option<&Path>)->Option<PathBuf>{dir.ends_with("Contents/MacOS").then(||home.map(|home|home.join("Library/Application Support").join(setup::APP_NAME))).flatten()}
/// The folder the settings and images are kept in: the program's own (for a build run from
/// `target\debug` or `target\release`, the project's; for a macOS app, `bundle_data_folder`).
/// The folder it was started in, should the system not say where the program is.
pub(crate) fn app_root()->PathBuf{
 let Some(dir)=std::env::current_exe().ok().and_then(|exe|exe.parent().map(Path::to_path_buf)) else{return PathBuf::from(".")};
 if cfg!(target_os="macos"){if let Some(kept)=bundle_data_folder(&dir,std::env::var_os("HOME").map(PathBuf::from).as_deref()).filter(|kept|std::fs::create_dir_all(kept).is_ok()){return kept;}}
 let built=dir.file_name().is_some_and(|x|x=="debug"||x=="release")&&dir.parent().is_some_and(|x|x.ends_with("target"));
 if built{if let Some(project)=dir.parent().and_then(Path::parent){return project.to_path_buf();}}
 dir
}
/// The settings saved in `root`. A settings file that cannot be read as settings (damaged, or
/// written by a build with a value this one does not know) gives way to the copy the save before
/// it kept, settings.backup.json, and only when that cannot be read either to a first start's
/// settings. With no settings file at all it is a first start. Nothing is written here; `save`
/// puts the unreadable file aside before it writes.
fn load_settings(root:&Path)->(Settings,Option<String>){
 // `None`: no such file. `Some(None)`: there, but not settings this build can read.
 let read=|name:&str|match std::fs::read_to_string(root.join(name)){Ok(text)=>Some(serde_json::from_str::<Settings>(&text).ok()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>None,Err(_)=>Some(None)};
 let(mut s,warning)=match read("settings.json"){
  Some(Some(saved))=>(saved,None),
  None=>(Settings::first(Lang::system()),None),
  Some(None)=>match read("settings.backup.json"){
   Some(Some(before))=>(before,Some(t("저장 설정을 읽지 못해 직전에 저장한 설정으로 열었습니다.").into())),
   _=>(Settings::first(Lang::system()),Some(t("저장 설정을 읽지 못해 기본 설정으로 열었습니다.").into())),
  },
 };
 s.normalize();s.migrate();(s,warning)
}
fn load_image(path:&Path)->Result<image::RgbaImage,String>{
 let meta=std::fs::metadata(path).map_err(|e|e.to_string())?;if meta.len()>40*1024*1024{return Err(t("이미지는 40MB 이하로 선택해주세요.").into());}
 let dims=image::image_dimensions(path).map_err(|e|e.to_string())?;if dims.0>8192||dims.1>8192{return Err(t("이미지는 가로·세로 8192px 이하로 선택해주세요.").into());}
 image::open(path).map(|i|i.to_rgba8()).map_err(|e|e.to_string())
}
/// The longest side a picture is kept at. A background fills a 4K screen at 3840; a logo, drawn at
/// most about a thousand pixels wide (at 300 % on such a screen), has room to spare at 2048. A
/// larger picture is made smaller when it is chosen, so that it is quick to save and to open
/// again; one that an earlier build saved large is made smaller in memory each time it is opened.
const BACKGROUND_SIDE:u32=3840;
const LOGO_SIDE:u32=2048;
/// The slots of the three pictures: 0 the background, 1 the brand's logo, 2 the partner's.
fn side_for(slot:usize)->u32{if slot==0{BACKGROUND_SIDE}else{LOGO_SIDE}}
/// The picture no longer than `side` on its longer side, in its own proportions. Colours are
/// averaged by how opaque they are, so the see-through surroundings of a logo do not tint its edge.
fn fit_within(mut im:image::RgbaImage,side:u32)->image::RgbaImage{
 let(w,h)=im.dimensions();let longer=w.max(h);if longer<=side{return im;}
 let scaled=|v:u32|((v as f64*side as f64/longer as f64).round() as u32).max(1);
 for p in im.pixels_mut(){let a=p[3] as u32;for c in 0..3{p[c]=((p[c] as u32*a+127)/255) as u8;}}
 let mut small=image::imageops::thumbnail(&im,scaled(w),scaled(h));
 for p in small.pixels_mut(){let a=p[3] as u32;if a>0{for c in 0..3{p[c]=((p[c] as u32*255+a/2)/a).min(255) as u8;}}}
 small
}
/// A logo that stands out from what is behind it by less than this contrast ratio is lost on it, and
/// 자동 shows it white. (Red on the dark stage, about 3 : 1, keeps its colour; navy, about 1.2 : 1, and
/// dark green, about 1.1 : 1, do not.)
const LOGO_CONTRAST:f32=2.0;
/// The pictures the person chose, ready to draw: each file's texture with the mean colour of what is
/// not transparent in it (sRGB 0..1, weighted by opacity) and how opaque it is overall, and for each
/// logo a white copy.
#[derive(Default)]
pub(crate) struct Pictures{own:HashMap<String,TextureHandle>,white:HashMap<String,TextureHandle>,mean:HashMap<String,([f32;3],f32)>}
impl Pictures{
 fn get(&self,name:&str)->Option<&TextureHandle>{self.own.get(name)}
 fn contains_key(&self,name:&str)->bool{self.own.contains_key(name)}
 /// The logo `name` as it is drawn on a background of colour `behind` (`None`: not known, as under the
 /// floating timer, where the slides are), and whether that is its white copy.
 fn logo(&self,name:&str,tone:Tone,behind:Option<[f32;3]>)->Option<(&TextureHandle,bool)>{
  let own=self.own.get(name)?;
  let white=match tone{Tone::Own=>false,Tone::White=>true,Tone::Auto=>matches!((behind,self.mean.get(name)),(Some(bg),Some((logo,_))) if needs_white(*logo,bg))};
  Some(match self.white.get(name){Some(w) if white=>(w,true),_=>(own,false)})
 }
}
/// The mean colour of what is not transparent in a picture (sRGB 0..1, weighted by opacity) and its
/// mean opacity, from at most about 260 000 of its pixels.
fn mean_color(im:&image::RgbaImage)->([f32;3],f32){
 let px=im.as_raw().chunks_exact(4);let n=px.len();let step=(n/262_144).max(1);
 let(mut sum,mut alpha,mut count)=([0.0f64;3],0.0f64,0usize);
 for p in px.step_by(step){let a=p[3] as f64/255.0;for i in 0..3{sum[i]+=p[i] as f64/255.0*a;}alpha+=a;count+=1;}
 if alpha<=0.0{return([0.0;3],0.0);}
 (sum.map(|v|(v/alpha) as f32),(alpha/count as f64) as f32)
}
/// One channel through a colour transform: the multiplier in 1/256 (256 keeps the channel as it is),
/// the offset in colour units, the result kept within 0..255.
fn transform_channel(c:u8,multiplier:i32,offset:i32)->u8{((c as i32*multiplier>>8)+offset).clamp(0,255) as u8}
/// The picture with every pixel white and its transparency kept: multipliers 0 and offsets 255 on
/// red, green and blue, multiplier 256 and offset 0 on the alpha.
fn white_copy(im:&image::RgbaImage)->Vec<u8>{im.as_raw().chunks_exact(4).flat_map(|p|[transform_channel(p[0],0,255),transform_channel(p[1],0,255),transform_channel(p[2],0,255),transform_channel(p[3],256,0)]).collect()}
/// Relative luminance of an sRGB colour (0..1 per channel), as WCAG 2.1 defines it.
fn luminance(c:[f32;3])->f32{let lin=|v:f32|if v<=0.04045{v/12.92}else{((v+0.055)/1.055).powf(2.4)};0.2126*lin(c[0])+0.7152*lin(c[1])+0.0722*lin(c[2])}
/// Contrast ratio of two luminances, from 1 (none) to 21.
fn contrast(a:f32,b:f32)->f32{(a.max(b)+0.05)/(a.min(b)+0.05)}
/// Whether a logo of mean colour `logo` would get lost on a background of colour `behind`, so that
/// 자동 shows it white: the background is dark (white stands out on it more than black does) and the
/// logo stands out from it by less than `LOGO_CONTRAST`. A logo that already stands out keeps its
/// colours, and on a light background every logo keeps them.
fn needs_white(logo:[f32;3],behind:[f32;3])->bool{let bg=luminance(behind);contrast(1.0,bg)>contrast(0.0,bg)&&contrast(luminance(logo),bg)<LOGO_CONTRAST}
fn rgb(c:Color32)->[f32;3]{[c.r() as f32/255.0,c.g() as f32/255.0,c.b() as f32/255.0]}
impl Studio{
 fn bare(s:Settings,root:PathBuf,shot:Option<PathBuf>)->Self{let last_saved=serde_json::to_string(&s).unwrap();Self{clock:Clock::new(s.duration),s,epoch:Instant::now(),root,textures:Pictures::default(),tab:0,status:t("설정은 자동으로 저장됩니다").into(),last_saved,dirty_since:None,presentation:false,audience:false,audience_full:false,confirm_reset:false,prior_phase:Phase::Ready,shot,frames:0,shot_requested:false,screenshot_ok:false,
  overlay:false,hwnd:None,editor_rect:None,geometry:None,place_overlay:false,last_topmost:0.0,activity:Instant::now(),audience_activity:Instant::now(),bars:HashMap::new(),float_buttons:Vec::new(),gesture:None,flash_since:None,force_controls:false,pulse:None,auto_float:None,auto_back:None,auto_language:None,hotkeys:None,hotkey_failed:false,digit_hotkeys:None,digit_failed:false,hit_region:None,packer:None,wide_fonts:false,font_text:String::new(),panel_end:false,steady:false,settle_until:SETTLE_FRAMES,pointer_at:None}}
 fn new(cc:&eframe::CreationContext<'_>,shot:Option<PathBuf>,demo:Option<String>,floating:bool,language:Option<Lang>)->Self{
  let ctx=&cc.egui_ctx;
  let root=app_root();let(mut s,warning)=load_settings(&root);
  // `--lang`: this start's language whatever is saved.
  if let Some(l)=language{if let Some(event)=placeholder_in(&s.event,s.language,l){s.event=event.into();}s.language=l;}lang::set(s.language);theme(ctx,s.language);
  // `--demo a,b`: fixed states for checks (circle|bar|number, warning|over|hover, packer|packed: the 설치 파일 만들기 window).
  let demos:Vec<&str>=demo.as_deref().map(|d|d.split(',').collect()).unwrap_or_default();
  for d in &demos{match *d{"circle"=>s.mode=Mode::Circle,"bar"=>s.mode=Mode::Bar,"number"=>s.mode=Mode::Number,_=>{}}}
  let mut app=Self::bare(s,root,shot);app.steady=demo.is_some();if let Some(w)=warning{app.status=w;}if app.s.pin&&!floating{ctx.send_viewport_cmd(ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));}
  for d in &demos{match *d{"warning"=>app.clock.accumulated=245.0,"over"=>app.clock.accumulated=312.0,"hover"=>{app.clock.accumulated=95.0;app.force_controls=true;},"pulse"=>app.pulse=Some((5.0,7.5)),"float"=>app.auto_float=Some(90),"back"=>app.auto_back=Some(600),"to-ko"|"to-en"|"to-zh"|"to-ja"=>app.auto_language=Lang::from_code(&d[3..]),"timetab"=>app.tab=1,"floattab"=>app.tab=2,"panelend"=>app.panel_end=true,"packer"=>app.packer=Some(Packer{with_settings:true,made:None,error:None}),"packed"=>app.packer=Some(Packer{with_settings:true,made:Some(app.root.join(setup::SETUP_NAME)),error:None}),_=>{}}}
  // (A check's hidden window stays where it was put, beyond the screen.)
  app.overlay=floating;app.place_overlay=floating&&!demos.contains(&"hidden");app.reload_images(ctx);app
 }
 fn now(&self)->f64{self.epoch.elapsed().as_secs_f64()}
 /// A window is opening or changing its kind (floating timer, full screen, audience window):
 /// every frame is drawn for a little while, until Windows and the picture have caught up.
 fn settle(&mut self){self.settle_until=self.frames+SETTLE_FRAMES;}
 /// How long the picture on screen stays right; the window is drawn again only then, and at once
 /// when the mouse or a key does something (egui sees to that, and to its own fades). While the
 /// timer runs that is the next whole second, and often enough in between for the ring or bar to
 /// move without steps; besides, the moment the hover buttons are due to hide, a changed setting
 /// is due to be saved, the end blink turns, or the floating timer is put back on top. `None`:
 /// nothing changes by itself. A check's fixed states, a window settling or being placed and a
 /// drag are counted in frames or follow the mouse, and keep every frame.
 fn next_frame_in(&self)->Option<Duration>{
  if self.shot.is_some()||self.steady||self.frames<self.settle_until||self.place_overlay||self.geometry.is_some()||self.gesture.is_some(){return Some(FRAME);}
  let now=self.now();let mut wait=f64::INFINITY;
  if let Some(tick)=self.clock.next_change(now){
   wait=wait.min(tick);
   // The ring and the bar move all the time until the end, then stand still.
   if self.s.mode!=Mode::Number&&self.clock.elapsed(now)<self.clock.duration{wait=wait.min((self.clock.duration/GRAPH_STEPS).clamp(FRAME.as_secs_f64(),GRAPH_SLOWEST));}
  }
  if self.overlay{
   wait=wait.min(TOPMOST_EVERY-(now-self.last_topmost));if win::FOLLOWS_POINTER{wait=wait.min(POINTER_POLL);}
   if let Some(e)=self.flash_since.map(|t|t.elapsed().as_secs_f64()).filter(|e|*e<FLASH_SECS){wait=wait.min(0.5-e%0.5);}
  }
  for idle in [Some(self.activity),self.audience.then_some(self.audience_activity)].into_iter().flatten().map(|at|at.elapsed().as_secs_f64()){if idle<CONTROLS_IDLE{wait=wait.min(CONTROLS_IDLE-idle);}}
  if let Some(since)=self.dirty_since{wait=wait.min(SAVE_AFTER.as_secs_f64()-since.elapsed().as_secs_f64());}
  wait.is_finite().then(||Duration::from_secs_f64(wait.max(0.0))+FRAME_LATE)
 }
 fn started(&self)->bool{self.clock.qna||self.clock.elapsed(self.now())>0.0}
 fn controls_forced(&self)->bool{self.force_controls||self.pulse.is_some_and(|(from,to)|(from..to).contains(&self.now()))}
 fn control_state(&self)->controls::State{controls::State{running:self.clock.running(),started:self.started(),accent:self.s.accent()}}
 fn picture_name(&self,slot:usize)->&String{match slot{0=>&self.s.background,1=>&self.s.logo,_=>&self.s.logo2}}
 /// Puts a picture on screen for its slot: its texture, its mean colour and, for a logo, its white copy.
 fn show_image(&mut self,ctx:&Context,slot:usize,im:&image::RgbaImage){
  let name=self.picture_name(slot).clone();let size=[im.width() as usize,im.height() as usize];let pics=&mut self.textures;
  pics.own.insert(name.clone(),ctx.load_texture(&name,ColorImage::from_rgba_unmultiplied(size,im.as_raw()),TextureOptions::LINEAR));pics.mean.insert(name.clone(),mean_color(im));
  if slot>0{pics.white.insert(name.clone(),ctx.load_texture(format!("{name} (white)"),ColorImage::from_rgba_unmultiplied(size,&white_copy(im)),TextureOptions::LINEAR));}
 }
 /// Opens the saved picture of one slot; the other two stay as they are.
 fn reload_image(&mut self,ctx:&Context,slot:usize){let name=self.picture_name(slot);if name.is_empty(){return;}match load_image(&self.root.join(name)){Ok(im)=>self.show_image(ctx,slot,&fit_within(im,side_for(slot))),Err(e)=>self.status=tf!("이미지 불러오기 실패: {0}",e)}}
 fn reload_images(&mut self,ctx:&Context){self.textures=Pictures::default();for slot in 0..3{self.reload_image(ctx,slot);}}
 /// A picture the person chose: made no larger than its slot needs, saved as PNG in `assets`, and
 /// shown from what is in memory (the other pictures are not read again).
 fn pick_image(&mut self,ctx:&Context,slot:usize){if let Some(p)=rfd::FileDialog::new().set_title(t("사용할 이미지 선택")).add_filter(t("이미지"),&["png","jpg","jpeg","webp","bmp"]).pick_file(){match load_image(&p){Ok(im)=>{let im=fit_within(im,side_for(slot));let name=format!("assets/{}.png",["background","brand-logo","partner-logo"][slot]);let dest=self.root.join(&name);let result=std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e|e.to_string()).and_then(|_|im.save(&dest).map_err(|e|e.to_string()));match result{Ok(())=>{match slot{0=>self.s.background=name,1=>self.s.logo=name,_=>self.s.logo2=name};self.show_image(ctx,slot,&im);self.status=t("이미지를 적용했습니다").into();},Err(e)=>self.status=tf!("이미지 저장 실패: {0}",e)}},Err(e)=>self.status=tf!("이미지 선택 실패: {0}",e)}}}
 fn save(&mut self){if self.shot.is_some(){return;}let result=serde_json::to_string_pretty(&self.s).map_err(|e|e.to_string()).and_then(|text|{let path=self.root.join("settings.json");let tmp=self.root.join("settings.pending.json");std::fs::write(&tmp,text).map_err(|e|e.to_string())?;
  // The file about to be replaced becomes the backup. One that cannot be read as settings does not:
  // the backup may then be the only good copy (`load_settings` opened from it), so it stays, and
  // the unreadable file is kept under a name of its own.
  if path.exists(){let readable=std::fs::read_to_string(&path).is_ok_and(|old|serde_json::from_str::<Settings>(&old).is_ok());std::fs::copy(&path,self.root.join(if readable{"settings.backup.json"}else{UNREADABLE_NAME})).map_err(|e|e.to_string())?;}
  std::fs::rename(&tmp,&path).map_err(|e|e.to_string())});match result{Ok(())=>{self.last_saved=serde_json::to_string(&self.s).unwrap();self.status=t("모든 변경사항이 저장되었습니다").into();self.dirty_since=None;},Err(e)=>{self.status=tf!("저장 실패: {0}",e);self.dirty_since=Some(Instant::now());}}}
 /// Switches the interface to another language: texts, fonts and window title. An event name that is
 /// still the placeholder follows; anything the person typed stays.
 fn set_language(&mut self,ctx:&Context,language:Lang){
  if let Some(event)=placeholder_in(&self.s.event,self.s.language,language){self.s.event=event.into();}
  // (The new fonts may lack what the old ones had: the texts are looked at again.)
  self.s.language=language;lang::set(language);install_fonts(ctx,language,self.wide_fonts);self.font_text.clear();
  ctx.send_viewport_cmd(ViewportCommand::Title(t(if self.overlay{FLOAT_TITLE}else{EDITOR_TITLE}).into()));self.status=t("언어를 바꿨습니다").into();
 }
 /// Loads one more font once the brand, event name or subtitle has characters the fonts of this
 /// language lack (Chinese typed into a Korean interface, say).
 fn font_check(&mut self,ctx:&Context){
  let text=format!("{}\n{}\n{}",self.s.brand,self.s.event,self.s.subtitle);if text==self.font_text{return;}
  if !self.wide_fonts&&!ctx.fonts(|f|f.has_glyphs(&FontId::proportional(14.0),&text)){self.wide_fonts=true;install_fonts(ctx,self.s.language,true);}
  self.font_text=text;
 }
 fn reset(&mut self){self.clock.reset(self.s.duration);self.prior_phase=Phase::Ready;self.confirm_reset=false;self.flash_since=None;}
 fn toggle(&mut self){self.clock.toggle(self.now());}
 fn adjust(&mut self,d:f64){self.clock.adjust(d,self.now());if self.clock.elapsed(self.now())==0.0&&!self.clock.qna{self.s.duration=self.clock.duration;}}
 /// A press on the hover buttons. Reset needs no confirmation there (as in 1.1.0).
 fn apply_cmd(&mut self,cmd:controls::Cmd,ctx:&Context){match cmd{controls::Cmd::Reset=>self.reset(),controls::Cmd::Play=>if !self.clock.running(){self.toggle();},controls::Cmd::Pause=>if self.clock.running(){self.toggle();},controls::Cmd::Editor=>self.leave_floating(ctx),controls::Cmd::Quit=>ctx.send_viewport_cmd(ViewportCommand::Close)}}
 fn sound(&self,end:bool){if self.s.sound&&self.shot.is_none(){play_sound(self.s.volume,if end&&!self.s.sound_file.is_empty(){Some(self.root.join(&self.s.sound_file))}else{None},end);}}
 fn top(&mut self,ctx:&Context){TopBottomPanel::top("header").frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(24,17))).show(ctx,|ui|{ui.horizontal(|ui|{let(r,_)=ui.allocate_exact_size(vec2(28.0,28.0),Sense::hover());ui.painter().circle_stroke(r.center(),10.0,Stroke::new(3.0_f32,self.s.accent()));ui.painter().line_segment([r.center(),r.center()+vec2(0.0,-6.0)],Stroke::new(2.0_f32,self.s.accent()));ui.label(RichText::new("EVENT STUDIO").size(19.0).strong());ui.add_space(12.0);if ctx.screen_rect().width()>=HEADER_ROOMY{ui.label(RichText::new(t("브랜드 행사 타이머")).color(MUTED).size(12.0));}ui.with_layout(Layout::right_to_left(Align::Center),|ui|{
  let mut picked=self.s.language;language_picker(ui,&mut picked);if picked!=self.s.language{self.set_language(ctx,picked);}
  if ui.button(t("발표 화면 열기  ↗")).on_hover_text(t("별도 창을 다른 모니터로 옮길 수 있습니다")).clicked(){self.audience=true;self.settle();}if ui.button(t("전체 화면  F11")).clicked(){self.presentation=true;self.settle();ctx.send_viewport_cmd(ViewportCommand::Fullscreen(true));}
  if ui.add(Button::new(RichText::new(t("투명 타이머 실행")).color(self.s.accent()).strong()).stroke(Stroke::new(1.0_f32,self.s.accent()))).on_hover_text(t("배경 없이 다른 창 위에 늘 떠 있는 작은 타이머로 바꿉니다")).clicked(){self.enter_floating(ctx);}
  // (A setup file is Windows' way of handing the program on; a macOS app is copied as it is.)
  if cfg!(windows)&&ui.button(t("설치 파일 만들기")).on_hover_text(t("이 프로그램을 다른 컴퓨터에도 설치할 수 있는 파일 하나로 묶습니다")).clicked(){self.packer=Some(Packer{with_settings:true,made:None,error:None});}});});});}
 /// 설치 파일 만들기: this program as one setup file, with the settings and images on screen if wanted.
 fn packer_window(&mut self,ctx:&Context){
  let Some(mut p)=self.packer.take() else{return};let(mut open,mut close)=(true,false);
  Window::new(t("설치 파일 만들기")).collapsible(false).resizable(false).open(&mut open).anchor(Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui|{ui.set_width(430.0);
   ui.label(t("지금 쓰는 Event Studio를 설치 파일 하나로 묶습니다. 그 파일을 실행하면 프로그램이 설치되고 바탕 화면과 시작 메뉴에 바로 가기가 생깁니다. 관리자 권한은 필요 없습니다."));
   ui.add_space(4.0);ui.checkbox(&mut p.with_settings,t("지금 브랜드 · 행사 설정과 이미지도 함께 담기"));
   ui.label(RichText::new(t("담으면 설치한 컴퓨터에서 지금 이 화면 그대로 시작합니다. 그 컴퓨터에 이미 저장된 설정이 있으면 그대로 둘지 바꿀지 설치할 때 고릅니다.")).size(11.0).color(MUTED));ui.add_space(6.0);
   match p.made.clone(){
    Some(path)=>{ui.label(RichText::new(t("설치 파일을 만들었습니다")).color(self.s.accent()).strong());ui.label(RichText::new(path.display().to_string()).size(12.0).color(MUTED));ui.horizontal(|ui|{if ui.button(t("폴더 열기")).clicked(){setup::show_in_folder(&path);}if ui.button(t("하나 더 만들기")).clicked(){p.made=None;}if ui.button(t("닫기")).clicked(){close=true;}});},
    None=>if ui.add_sized([ui.available_width(),42.0],Button::new(RichText::new(t("저장할 곳 고르고 만들기")).color(BG).strong()).fill(self.s.accent()).corner_radius(9)).clicked(){
     if let Some(out)=rfd::FileDialog::new().set_title(t("설치 파일을 저장할 곳")).set_file_name(setup::SETUP_NAME).add_filter(t("설치 파일"),&["exe"]).save_file(){
      let out=if out.extension().is_some_and(|e|e.eq_ignore_ascii_case("exe")){out}else{out.with_extension("exe")};let text=serde_json::to_string_pretty(&self.s).unwrap();
      match setup::make_setup_here(&out,p.with_settings.then_some((text.as_str(),self.root.as_path()))){Ok(_)=>{p.made=Some(out);p.error=None;self.status=t("설치 파일을 만들었습니다").into();},Err(e)=>p.error=Some(e)}}},
   }
   if let Some(e)=&p.error{ui.label(RichText::new(e).color(END).size(12.0));}
  });
  if open&&!close{self.packer=Some(p);}
 }
 fn controls(&mut self,ctx:&Context){TopBottomPanel::bottom("transport").frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(24,16))).show(ctx,|ui|{ui.horizontal(|ui|{
  let label=if self.clock.running(){t("Ⅱ   일시정지")}else if self.clock.elapsed(self.now())>0.0{t("▶   이어하기")}else{t("▶   시작하기")};if ui.add_sized([153.0,46.0],Button::new(RichText::new(label).color(BG).strong()).fill(self.s.accent()).corner_radius(9)).clicked(){self.toggle();}if ui.add_sized([84.0,46.0],Button::new(t("초기화"))).clicked(){if self.clock.elapsed(self.now())>0.0{self.confirm_reset=true}else{self.reset();}}ui.add_space(8.0);
  for(txt,delta)in[(t("−1분"),-60.0),(t("−10초"),-10.0),(t("+10초"),10.0),(t("+1분"),60.0)]{if ui.button(txt).clicked(){self.adjust(delta);}}
  if self.s.qna_enabled&&ui.add_enabled(!self.clock.qna,Button::new(t("Q&A 전환"))).clicked(){self.clock.switch_qna(self.s.qna_duration,self.s.carry,self.now());}
  ui.with_layout(Layout::right_to_left(Align::Center),|ui|{ui.label(RichText::new(t("SPACE  시작 / 정지")).color(MUTED).size(11.0));});});ui.add_space(4.0);ui.horizontal(|ui|{ui.label(RichText::new(&self.status).color(MUTED).size(11.0));ui.with_layout(Layout::right_to_left(Align::Center),|ui|{ui.label(RichText::new(t("로컬 저장 · 인터넷 연결 불필요")).color(MUTED).size(11.0));
   if ui.link(RichText::new(t("오픈소스 라이선스")).size(11.0)).on_hover_text(t("함께 쓰인 오픈소스 라이브러리와 글꼴의 라이선스를 파일로 저장해 보여 줍니다")).clicked(){self.show_licenses();}});});});}
 /// Writes the licences of the libraries and fonts the program is built with beside the settings
 /// and shows the file in its folder: they are part of every copy of the program, installed or not.
 fn show_licenses(&mut self){let file=self.root.join(setup::LICENSES_NAME);match std::fs::write(&file,setup::LICENSES){Ok(())=>{setup::show_in_folder(&file);self.status=tf!("오픈소스 라이선스를 저장했습니다: {0}",file.display());},Err(e)=>self.status=tf!("저장 실패: {0}",e)}}
 fn inspector(&mut self,ctx:&Context){SidePanel::right("inspector").resizable(false).exact_width(340.0).frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(22,20))).show(ctx,|ui|{
  ui.horizontal(|ui|{ui.heading(t("화면 설정"));ui.with_layout(Layout::right_to_left(Align::Center),|ui|{ui.label(RichText::new("LIVE").size(10.0).color(self.s.accent()));});});ui.label(RichText::new(t("변경하면 미리보기에 바로 반영됩니다")).color(MUTED).size(11.0));ui.add_space(5.0);ui.horizontal(|ui|{ui.spacing_mut().button_padding=vec2(9.0,9.0);ui.spacing_mut().item_spacing.x=6.0;ui.selectable_value(&mut self.tab,0,t("브랜드·이미지"));ui.selectable_value(&mut self.tab,1,t("시간·진행"));ui.selectable_value(&mut self.tab,2,t("투명 타이머"));});ui.separator();
  ScrollArea::vertical().auto_shrink([false,false]).show(ui,|ui|{match self.tab{0=>self.brand_settings(ui,ctx),1=>self.time_settings(ui,ctx),_=>self.floating_settings(ui,ctx)}if self.panel_end{ui.scroll_to_cursor(Some(Align::BOTTOM));}});
 });}
 fn brand_settings(&mut self,ui:&mut Ui,ctx:&Context){
  field_label(ui,t("브랜드 이름"));ui.add(TextEdit::singleline(&mut self.s.brand).desired_width(f32::INFINITY).hint_text(t("브랜드 이름 입력")).char_limit(80));field_label(ui,t("행사명"));ui.add(TextEdit::singleline(&mut self.s.event).desired_width(f32::INFINITY).hint_text(t("행사명을 입력하세요")).char_limit(180));field_label(ui,t("부제 / 세션명"));ui.add(TextEdit::singleline(&mut self.s.subtitle).desired_width(f32::INFINITY).hint_text(t("발표자 또는 세션명")).char_limit(180));
  ui.add_space(7.0);field_label(ui,t("브랜드 컬러"));ui.horizontal(|ui|{for col in [[132,226,190],[98,172,255],[191,153,255],[255,185,112],[246,126,145]]{let c=Color32::from_rgb(col[0],col[1],col[2]);let(r,res)=ui.allocate_exact_size(vec2(29.0,29.0),Sense::click());ui.painter().circle_filled(r.center(),10.0,c);if self.s.accent==col{ui.painter().circle_stroke(r.center(),13.0,Stroke::new(1.5_f32,WHITE));}if res.clicked(){self.s.accent=col;}}ui.color_edit_button_srgb(&mut self.s.accent);});
  ui.add_space(7.0);field_label(ui,t("브랜드 로고"));self.asset_row(ui,ctx,1);field_label(ui,t("파트너 로고 (선택)"));self.asset_row(ui,ctx,2);
  if !self.s.logo.is_empty()||!self.s.logo2.is_empty(){let names=[t("왼쪽 위"),t("가운데 위"),t("오른쪽 위"),t("왼쪽 아래"),t("가운데 아래"),t("오른쪽 아래")];ComboBox::from_id_salt("logo_position").selected_text(names[self.s.logo_position]).show_ui(ui,|ui|{for(i,n)in names.iter().enumerate(){ui.selectable_value(&mut self.s.logo_position,i,*n);}});ui.add(Slider::new(&mut self.s.logo_size,20.0..=300.0).suffix("%").text(t("크기")));
   ui.horizontal_wrapped(|ui|{ui.label(t("로고 색"));for(tone,label)in[(Tone::Auto,t("자동")),(Tone::Own,t("원래 색")),(Tone::White,t("흰색"))]{ui.selectable_value(&mut self.s.logo_tone,tone,label);}});ui.label(RichText::new(t("자동: 어두운 화면에서 안 보이는 로고는 흰색으로 표시")).color(MUTED).size(10.0));}
  ui.add_space(6.0);field_label(ui,t("행사 배경 이미지"));self.asset_row(ui,ctx,0);ui.label(RichText::new(t("PNG · JPG · WebP · BMP / 최대 40MB")).color(MUTED).size(10.0));if !self.s.background.is_empty(){ui.horizontal(|ui|{ui.selectable_value(&mut self.s.cover,true,t("화면 채우기"));ui.selectable_value(&mut self.s.cover,false,t("전체 보이기"));});ui.add(Slider::new(&mut self.s.dim,0.0..=1.0).text(t("어둡게")));}ui.add_space(8.0);ui.label(RichText::new(t("직접 제작했거나 사용 권한이 있는 이미지를 넣어주세요.")).color(MUTED).size(11.0));
 }
 fn time_settings(&mut self,ui:&mut Ui,ctx:&Context){
  field_label(ui,t("발표 시간"));let before=self.s.duration;duration_editor(ui,&mut self.s.duration,"duration");if self.s.duration!=before{self.duration_changed();}
  // Five quick choices on one line in every language (they wrap if a font makes them wider).
  ui.horizontal_wrapped(|ui|{ui.spacing_mut().item_spacing.x=6.0;ui.spacing_mut().button_padding.x=6.0;for m in [3,5,10,15,20]{if ui.small_button(tf!("{0}분",m)).clicked(){self.s.duration=(m*60) as f64;self.duration_changed();}}});
  ui.add_space(4.0);field_label(ui,t("주의 알림 시점 (남은 시간)"));duration_editor(ui,&mut self.s.warning,"warning");ui.label(RichText::new(t("0초는 주의 알림 끄기")).size(11.0).color(MUTED));ui.checkbox(&mut self.s.count_up,t("경과시간 표시 (카운트업)"));ui.checkbox(&mut self.s.overtime,t("종료 후 초과시간 표시"));ui.checkbox(&mut self.s.seconds_only,t("분:초 대신 초 단위로 표시"));ui.separator();ui.checkbox(&mut self.s.qna_enabled,t("발표 후 Q&A 자동 시작"));if self.s.qna_enabled{field_label(ui,t("Q&A 시간"));duration_editor(ui,&mut self.s.qna_duration,"qna");ui.checkbox(&mut self.s.carry,t("일찍 전환하면 남은 발표시간 이월"));}
  ui.separator();ui.checkbox(&mut self.s.sound,t("주의 / 종료 알림음"));if self.s.sound{ui.add(Slider::new(&mut self.s.volume,0.0..=1.0).text(t("음량")));ui.horizontal(|ui|{if ui.button(t("소리 테스트")).clicked(){self.sound(true);}if ui.button(t("알림음 선택")).clicked(){if let Some(p)=rfd::FileDialog::new().add_filter(t("오디오"),&["wav","mp3"]).pick_file(){let ext=p.extension().and_then(|s|s.to_str()).unwrap_or("wav");let name=format!("assets/alert.{ext}");let dst=self.root.join(&name);let _=std::fs::create_dir_all(dst.parent().unwrap());match std::fs::copy(p,dst){Ok(_)=>{self.s.sound_file=name;self.status=t("종료 알림음을 선택했습니다").into();},Err(e)=>self.status=e.to_string()}}}});if !self.s.sound_file.is_empty()&&ui.small_button(t("기본 알림음 복원")).clicked(){self.s.sound_file.clear();}}
  ui.separator();if ui.checkbox(&mut self.s.pin,t("항상 위에 표시")).changed(){ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if self.s.pin{egui::WindowLevel::AlwaysOnTop}else{egui::WindowLevel::Normal}));}ui.add_space(8.0);ui.label(RichText::new(t("단축키\nSpace  시작 / 일시정지\nR  초기화    Q  Q&A 전환\n1 / 2 / 3  표시 방식\nF11  전체 화면    Esc  돌아가기")).size(11.0).color(MUTED));
 }
 fn floating_settings(&mut self,ui:&mut Ui,ctx:&Context){
  ui.label(RichText::new(t("배경 없이 다른 창 위에 늘 떠 있는 작은 타이머입니다. 발표 슬라이드 위에 올려 쓰세요. 떠 있는 동안에는 작업 표시줄에 나오지 않으니, 돌아올 때는 타이머에 마우스를 올려 톱니 버튼을 누르세요.")).size(12.0).color(MUTED));ui.add_space(6.0);
  if ui.add_sized([ui.available_width(),44.0],Button::new(RichText::new(t("투명 타이머 실행")).color(BG).strong()).fill(self.s.accent()).corner_radius(9)).clicked(){self.enter_floating(ctx);}
  ui.add_space(4.0);ui.checkbox(&mut self.s.overlay_start,t("앱을 열면 투명 타이머로 바로 시작"));
  ui.add_space(4.0);ui.checkbox(&mut self.s.overlay_scene,t("브랜드 · 행사명 · 로고도 함께 표시"));
  ui.label(RichText::new(t("배경은 항상 투명합니다. 행사 배경 이미지는 미리보기 · 전체 화면 · 발표 화면에만 나옵니다.")).size(11.0).color(MUTED));
  ui.add_space(4.0);field_label(ui,t("어두운 바탕 (0% = 완전히 투명)"));let mut pct=self.s.overlay_backdrop*100.0;if ui.add(Slider::new(&mut pct,0.0..=BACKDROP_MAX*100.0).suffix("%")).changed(){self.s.overlay_backdrop=(pct/100.0).clamp(0.0,BACKDROP_MAX);}
  if ui.button(t("다음에 화면 오른쪽 위에서 열기")).clicked(){self.s.overlay_top_right=true;self.status=t("투명 타이머를 오른쪽 위에서 엽니다").into();}
  ui.separator();ui.checkbox(&mut self.s.hotkeys,t("전역 단축키 사용"));ui.label(RichText::new(t("Ctrl+Alt+Space 시작 / 멈춤 · Ctrl+Alt+R 리셋 · Ctrl+Alt+Q Q&A\n투명 타이머가 떠 있을 때는 Ctrl+1 시작 / 멈춤 · Ctrl+2 리셋 · Ctrl+3 Q&A\n다른 프로그램을 보고 있어도 작동")).size(11.0).color(MUTED));
  ui.separator();field_label(ui,t("사용법"));ui.label(RichText::new(t("마우스를 올리면 리셋 · 재시작 · 멈춤 버튼이 나타납니다\n끌어서 이동 · 클릭 시작/멈춤 · 오른쪽 클릭 리셋\n오른쪽 아래 모서리를 끌거나 Ctrl+휠로 크기 조절\n톱니 버튼: 편집 화면으로 · X 버튼: 앱 종료\n버튼을 눌러도 발표 프로그램의 키보드 입력은 그대로입니다\n떠 있는 동안에는 작업 표시줄과 Alt+Tab에 나오지 않습니다")).size(11.0).color(MUTED));
 }
 fn duration_changed(&mut self){if !self.clock.running()&&self.clock.elapsed(self.now())==0.0{self.clock.duration=self.s.duration;}else{self.status=t("변경한 시간은 초기화 후 적용됩니다").into();}}
 fn asset_row(&mut self,ui:&mut Ui,ctx:&Context,slot:usize){let name=match slot{0=>self.s.background.clone(),1=>self.s.logo.clone(),_=>self.s.logo2.clone()};Frame::new().fill(BG).corner_radius(8).inner_margin(10).show(ui,|ui|{ui.set_min_width(258.0);ui.horizontal(|ui|{let shown=if slot==0{self.textures.get(&name)}else{self.textures.logo(&name,self.s.logo_tone,Some(rgb(BG))).map(|l|l.0)};if let Some(t)=shown{ui.add(Image::new(t).fit_to_exact_size(vec2(34.0,30.0)));}else{ui.label(RichText::new("＋").size(21.0).color(MUTED));}if ui.button(if name.is_empty(){t("이미지 선택")}else{t("이미지 변경")}).clicked(){self.pick_image(ctx,slot);}if !name.is_empty()&&ui.small_button(t("제거")).clicked(){match slot{0=>self.s.background.clear(),1=>self.s.logo.clear(),_=>self.s.logo2.clear()};}});});}
 fn workspace(&mut self,ctx:&Context){CentralPanel::default().frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(28,23))).show(ctx,|ui|{
  ui.horizontal(|ui|{ui.label(RichText::new(t("행사 화면 미리보기")).size(17.0).strong());ui.with_layout(Layout::right_to_left(Align::Center),|ui|{ui.label(RichText::new(t("실시간 반영")).color(MUTED).size(11.0));let(r,_)=ui.allocate_exact_size(vec2(7.0,7.0),Sense::hover());ui.painter().circle_filled(r.center(),3.0,self.s.accent());});});ui.add_space(10.0);
  ui.horizontal(|ui|{for(mode,label)in[(Mode::Circle,t("원그래프")),(Mode::Bar,t("막대그래프")),(Mode::Number,t("123  숫자"))]{let active=self.s.mode==mode;let b=Button::new(RichText::new(label).color(if active{self.s.accent()}else{MUTED})).fill(if active{Color32::from_rgb(31,56,51)}else{PANEL}).corner_radius(8);if ui.add_sized([126.0,37.0],b).clicked(){self.s.mode=mode;}}});ui.add_space(16.0);
  let size=vec2(ui.available_width(),(ui.available_height()-63.0).max(220.0));let(rect,_)=ui.allocate_exact_size(size,Sense::hover());let view=self.clock.view(self.now(),self.s.warning,self.s.count_up,self.s.overtime,self.s.seconds_only);let timer_rect=scene::render(ui.painter(),rect,&self.s,&view,&self.textures,self.clock.qna,self.clock.running());if let Some(cmd)=self.hover_bar(ui,"preview",rect,timer_rect,self.activity){self.apply_cmd(cmd,ctx);}ui.add_space(14.0);
  ui.horizontal(|ui|{ui.label(RichText::new(if self.clock.qna{"Q&A"}else{"PRESENTATION"}).size(10.0).color(self.s.accent()));ui.label(RichText::new(tf!("설정 {0}  ·  경과 {1}",timer::format_seconds(self.clock.duration as u64),timer::format_seconds(view.elapsed.floor() as u64))).size(12.0).color(MUTED));ui.with_layout(Layout::right_to_left(Align::Center),|ui|{ui.label(RichText::new(t("마우스를 올리면 리셋 · 재시작 · 멈춤 버튼")).size(11.0).color(MUTED));});});
 });}
 /// The stage's hover buttons: shown while the mouse is over `area` and moved in the last
 /// 3 s (app.js wake), or rests on the bar; placed on the bottom edge of the timer.
 fn hover_bar(&mut self,ui:&Ui,salt:&'static str,area:Rect,timer_rect:Rect,activity:Instant)->Option<controls::Cmd>{
  let hover=ui.ctx().input(|i|i.pointer.hover_pos()).filter(|p|area.contains(*p));let on_bar=hover.is_some_and(|h|self.bars.get(salt).is_some_and(|r|r.contains(h)));
  let show=self.controls_forced()||on_bar||(hover.is_some()&&activity.elapsed().as_secs_f64()<CONTROLS_IDLE);let o=ui.ctx().animate_bool_with_time(ui.id().with(("controls",salt)),show,CONTROLS_FADE);
  let anchor=pos2(timer_rect.center().x,(timer_rect.bottom()+20.0).min(area.bottom()-6.0));let b=controls::bar(ui,salt,area,anchor,controls::STAGE,self.control_state(),o);self.bars.insert(salt,b.rect);b.pressed
 }
 fn keys(&mut self,ctx:&Context){if self.overlay{self.floating_keys(ctx);return;}if ctx.input(|i|i.key_pressed(Key::Escape)){self.presentation=false;self.settle();ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));}if fullscreen_key(ctx){self.presentation=!self.presentation;self.settle();ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.presentation));}if ctx.wants_keyboard_input(){return;}if ctx.input(|i|i.key_pressed(Key::Space)){self.toggle();}if ctx.input(|i|i.key_pressed(Key::R)){if self.clock.elapsed(self.now())>0.0{self.confirm_reset=true}else{self.reset();}}if ctx.input(|i|i.key_pressed(Key::Q))&&self.s.qna_enabled{self.clock.switch_qna(self.s.qna_duration,self.s.carry,self.now());}for(key,mode)in[(Key::Num1,Mode::Circle),(Key::Num2,Mode::Bar),(Key::Num3,Mode::Number)]{if ctx.input(|i|i.key_pressed(key)){self.s.mode=mode;}}}
 /// Keys while the floating timer has the keyboard (app.js KEYS): Space, R, arrows ±1 min / ±10 s, 1–3, S/F2/Esc editor.
 fn floating_keys(&mut self,ctx:&Context){let pressed=|k|ctx.input(|i|i.key_pressed(k));if pressed(Key::Space){self.toggle();}if pressed(Key::R){self.reset();}if pressed(Key::Q)&&self.s.qna_enabled{self.clock.switch_qna(self.s.qna_duration,self.s.carry,self.now());}
  for(key,d)in[(Key::ArrowUp,60.0),(Key::ArrowDown,-60.0),(Key::ArrowLeft,-10.0),(Key::ArrowRight,10.0)]{if pressed(key){self.adjust(d);}}for(key,mode)in[(Key::Num1,Mode::Circle),(Key::Num2,Mode::Bar),(Key::Num3,Mode::Number)]{if pressed(key){self.s.mode=mode;}}
  if pressed(Key::Escape)||pressed(Key::S)||pressed(Key::F2){self.leave_floating(ctx);}}
 /// Where the floating timer opens: last place if it is still on a screen, else the
 /// top-right of the work area under the window (placement.js pickBounds / topRight).
 fn floating_target(&self,ctx:&Context,h:isize)->win::PxRect{
  let ppp=win::units_per_point(ctx);let px=|v:f32|(v*ppp).round() as i32;let cur=win::window_rect(h);let(cx,cy)=cur.map_or((0,0),|r|(r.x+r.w/2,r.y+r.h/2));let saved=self.s.overlay_rect.map(win::PxRect::from_array);
  let(w,hh)=saved.map_or((px(FLOAT_SIZE.x),px(FLOAT_SIZE.y)),|r|(r.w.max(px(FLOAT_MIN.x)),r.h.max(px(FLOAT_MIN.y))));
  if let Some(r)=saved.filter(|r|!self.s.overlay_top_right&&win::on_screen(r.x+r.w/2,r.y+10)){return win::PxRect{x:r.x,y:r.y,w,h:hh};}
  let area=win::work_area(cx,cy).unwrap_or(win::PxRect{x:0,y:0,w:px(1920.0),h:px(1040.0)});win::top_right(area,w.min(area.w),hh.min(area.h),px(FLOAT_MARGIN))
 }
 fn editor_default(&self,ctx:&Context)->win::PxRect{let ppp=win::units_per_point(ctx);let c=self.hwnd.and_then(win::window_rect).map_or((0,0),|r|(r.x+r.w/2,r.y+r.h/2));let a=win::work_area(c.0,c.1).unwrap_or(win::PxRect{x:0,y:0,w:1920,h:1040});let w=((EDITOR_SIZE.x*ppp) as i32).min(a.w);let h=((EDITOR_SIZE.y*ppp) as i32).min(a.h);win::PxRect{x:a.x+(a.w-w)/2,y:a.y+(a.h-h)/2,w,h}}
 /// Turns the window into the floating timer: no frame, no background, always on top.
 fn enter_floating(&mut self,ctx:&Context){
  if self.overlay{return;}let Some(h)=self.hwnd else{self.status=t("이 컴퓨터에서는 투명 타이머를 열 수 없습니다").into();return;};
  if self.presentation{self.presentation=false;ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));}
  self.editor_rect=win::window_rect(h);let target=self.floating_target(ctx,h);
  for cmd in [ViewportCommand::Maximized(false),ViewportCommand::MinInnerSize(FLOAT_MIN),ViewportCommand::Decorations(false),ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop),ViewportCommand::Title(t(FLOAT_TITLE).into())]{ctx.send_viewport_cmd(cmd);}
  self.geometry=Some((target,self.frames+1));self.settle();self.overlay=true;self.confirm_reset=false;self.activity=Instant::now();self.status=t("투명 타이머: 마우스를 올리면 버튼이 나타납니다").into();
 }
 /// Back to the editor where it was.
 fn leave_floating(&mut self,ctx:&Context){
  if !self.overlay{return;}self.overlay=false;self.gesture=None;self.pointer_at=None;let target=self.editor_rect.unwrap_or_else(||self.editor_default(ctx));if let Some(h)=self.hwnd{self.set_hit_region(h,None);win::set_floating(h,false);}
  for cmd in [ViewportCommand::Decorations(true),ViewportCommand::WindowLevel(if self.s.pin{egui::WindowLevel::AlwaysOnTop}else{egui::WindowLevel::Normal}),ViewportCommand::MinInnerSize(EDITOR_MIN),ViewportCommand::Title(t(EDITOR_TITLE).into()),ViewportCommand::Focus]{ctx.send_viewport_cmd(cmd);}
  self.geometry=Some((target,self.frames+1));self.settle();
 }
 /// Applies a pending window rect for a few frames (after the frame/size commands landed),
 /// then keeps the floating timer on top and click-without-focus, re-asserted every second.
 fn window_upkeep(&mut self,ctx:&Context){
  let Some(h)=self.hwnd else{return};if self.place_overlay{self.place_overlay=false;self.geometry=Some((self.floating_target(ctx,h),self.frames));}
  if let Some((r,from))=self.geometry{if self.frames>=from{win::set_rect(h,r);}if self.frames>=from+2{self.geometry=None;if self.overlay{win::set_floating(h,true);win::keep_on_top(h);self.remember_floating(ctx);}}}
  if self.overlay&&self.gesture.is_none()&&self.now()-self.last_topmost>=TOPMOST_EVERY{self.last_topmost=self.now();win::keep_on_top(h);win::set_floating(h,true);}
 }
 /// Sets which parts of the window take the mouse; only touches the window when that changes.
 fn set_hit_region(&mut self,h:isize,region:Option<Vec<win::HitShape>>){if region!=self.hit_region{win::set_hit_region(h,region.as_deref());self.hit_region=region;}}
 /// Keeps where the floating timer is, and whether that is still the top-right spot (main.js saveBounds).
 fn remember_floating(&mut self,ctx:&Context){let Some(r)=self.hwnd.and_then(win::window_rect) else{return};self.s.overlay_rect=Some(r.to_array());let m=(FLOAT_MARGIN*win::units_per_point(ctx)).round() as i32;self.s.overlay_top_right=win::work_area(r.x+r.w/2,r.y+r.h/2).is_some_and(|a|{let t=win::top_right(a,r.w,r.h,m);(r.x-t.x).abs()<=2&&(r.y-t.y).abs()<=2});}
 /// Ctrl+wheel: scale around the centre, within the screen (main.js window:zoom).
 fn zoom_floating(&mut self,ctx:&Context,h:isize,f:f32){let Some(r)=win::window_rect(h) else{return};let ppp=win::units_per_point(ctx);let a=win::work_area(r.x+r.w/2,r.y+r.h/2).unwrap_or(win::PxRect{x:r.x,y:r.y,w:i32::MAX/4,h:i32::MAX/4});let w=((r.w as f32*f).round() as i32).max((FLOAT_MIN.x*ppp).round() as i32).min(a.w);let hh=((r.h as f32*f).round() as i32).max((FLOAT_MIN.y*ppp).round() as i32).min(a.h);win::set_rect(h,win::PxRect{x:r.x+(r.w-w)/2,y:r.y+(r.h-hh)/2,w,h:hh});self.remember_floating(ctx);}
 fn start_gesture(&mut self,kind:GestureKind,h:isize){if let Some(rect)=win::window_rect(h){self.gesture=Some(Gesture{kind,cursor:win::cursor(),rect,moved:false});}}
 /// The floating timer: drawn on nothing, buttons on hover, moved and sized with the mouse.
 fn floating(&mut self,ctx:&Context){
  let ppp=win::units_per_point(ctx);let(hover,pressed,secondary,zoom,origin)=ctx.input(|i|(i.pointer.hover_pos(),i.pointer.primary_pressed(),i.pointer.secondary_pressed(),i.zoom_delta(),i.pointer.press_origin()));
  let view=self.clock.view(self.now(),self.s.warning,self.s.count_up,self.s.overtime,self.s.seconds_only);let running=self.clock.running();
  let status=overlay::status_text(&view,self.s.count_up,running,self.started(),self.clock.qna,self.clock.duration);
  let blink=self.flash_since.is_some_and(|t|{let e=t.elapsed().as_secs_f64();e<FLASH_SECS&&e.fract()>=0.5});
  let(mut bar,mut grip,mut cmd,mut solid,mut shown)=(Rect::NOTHING,Rect::NOTHING,None,Vec::new(),false);
  CentralPanel::default().frame(Frame::NONE).show(ctx,|ui|{
   let rect=ui.max_rect();let mut p=ui.painter().clone();if blink{p.set_opacity(BLINK_OPACITY);}
   solid=overlay::paint(&p,rect,&self.s,&view,&status,&self.textures);
   let on_bar=hover.is_some_and(|h|self.bars.get("float").is_some_and(|r|r.contains(h)));let show=self.controls_forced()||self.gesture.is_some()||on_bar||(hover.is_some()&&self.activity.elapsed().as_secs_f64()<CONTROLS_IDLE);
   let o=ctx.animate_bool_with_time(Id::new("float_controls"),show,CONTROLS_FADE);shown=o>0.01;
   // style.css body.overlay #controls { bottom: 4px }, kept clear of the resize corner.
   let b=controls::bar(ui,"float",rect.shrink2(vec2(GRIP,0.0)),pos2(rect.center().x,rect.bottom()-4.0),controls::FLOATING,self.control_state(),o);self.bars.insert("float",b.rect);cmd=b.pressed;if o>0.01{bar=b.rect;}self.float_buttons=b.buttons;
   grip=Rect::from_min_size(rect.right_bottom()-Vec2::splat(GRIP),Vec2::splat(GRIP));if o>0.01{controls::grip(ui.painter(),grip,o);}
  });
  if let Some(h)=self.hwnd{
   // Empty pixels pass the mouse to the slides behind, as on a per-pixel-alpha layered window;
   // the whole window takes it while the buttons show or a drag runs.
   let region=(self.gesture.is_none()&&!shown).then(||solid.iter().map(|s|s.to_px(ppp)).collect());self.set_hit_region(h,region);
   if self.gesture.is_none()&&pressed{if let Some(pos)=origin{if grip.contains(pos){self.start_gesture(GestureKind::Resize,h);}else if !bar.contains(pos){self.start_gesture(GestureKind::Move,h);}}}
   if let Some(mut g)=self.gesture{
    let(cx,cy)=win::cursor();let(dx,dy)=(cx-g.cursor.0,cy-g.cursor.1);if g.kind==GestureKind::Resize||((dx*dx+dy*dy) as f32).sqrt()>=DRAG_PX*ppp{g.moved=true;}
    if g.moved{let r=g.rect;let next=match g.kind{GestureKind::Move=>win::PxRect{x:r.x+dx,y:r.y+dy,..r},GestureKind::Resize=>win::PxRect{w:(r.w+dx).max((FLOAT_MIN.x*ppp).round() as i32),h:(r.h+dy).max((FLOAT_MIN.y*ppp).round() as i32),..r}};win::set_rect(h,next);ctx.set_cursor_icon(if g.kind==GestureKind::Move{CursorIcon::Grabbing}else{CursorIcon::ResizeNwSe});}
    // app.js pointerup: a press that didn't move starts or pauses the timer.
    if win::left_button_down(){self.gesture=Some(g);}else{self.gesture=None;if g.moved{self.remember_floating(ctx);}else if g.kind==GestureKind::Move{self.toggle();}}
   }else if hover.is_some_and(|p|grip.contains(p)){ctx.set_cursor_icon(CursorIcon::ResizeNwSe);}
   if zoom!=1.0&&hover.is_some()&&self.gesture.is_none(){self.zoom_floating(ctx,h,if zoom>1.0{ZOOM_STEP}else{1.0/ZOOM_STEP});}
  }
  // app.js pointerdown: the right button resets.
  if secondary&&origin.is_some_and(|p|!bar.contains(p)){self.reset();}
  if let Some(c)=cmd{self.apply_cmd(c,ctx);}
  // macOS: a click brought this app to the front; the slide show gets the keyboard back (win_mac.rs).
  if self.overlay&&self.gesture.is_none(){win::return_focus();}
 }
 /// Optional system-wide keys: Ctrl+Alt+Space / R / Q all the time (1.1.0's global hotkeys), and
 /// while the floating timer is up also Ctrl+1/2/3.
 fn hotkey_sync(&mut self,ctx:&Context){
  if self.shot.is_some(){return;}
  let floating=self.overlay;
  for(slot,failed,want,keys)in[(&mut self.hotkeys,&mut self.hotkey_failed,self.s.hotkeys,&win::STUDIO_KEYS),(&mut self.digit_hotkeys,&mut self.digit_failed,self.s.hotkeys&&floating,&win::DIGIT_KEYS)]{
   if want&&slot.is_none()&&!*failed{let(tx,rx)=std::sync::mpsc::channel();let c=ctx.clone();match win::start_hotkeys(keys,move|k|{let _=tx.send(k);c.request_repaint();}){Ok(h)=>*slot=Some((h,rx)),Err(e)=>{*failed=true;self.status=e;}}}
   else if !want{*slot=None;if !self.s.hotkeys{*failed=false;}}
  }
  let keys:Vec<u8>=[&self.hotkeys,&self.digit_hotkeys].iter().filter_map(|s|s.as_ref()).flat_map(|(_,rx)|rx.try_iter().collect::<Vec<_>>()).collect();
  for k in keys{match k{win::HOT_TOGGLE=>self.toggle(),win::HOT_RESET=>self.reset(),win::HOT_QNA if self.s.qna_enabled=>self.clock.switch_qna(self.s.qna_duration,self.s.carry,self.now()),_=>{}}}
 }
}
/// F11 switches full screen; on macOS, where F11 belongs to the system, so does its own Ctrl+Cmd+F.
fn fullscreen_key(ctx:&Context)->bool{ctx.input(|i|i.key_pressed(Key::F11))||(cfg!(target_os="macos")&&ctx.input_mut(|i|i.consume_shortcut(&KeyboardShortcut::new(Modifiers::MAC_CMD|Modifiers::CTRL,Key::F))))}
/// The icon of the program's windows (the same drawing build.rs puts inside the program file).
/// (macOS shows it in the Dock, large.)
pub(crate) fn window_icon()->egui::IconData{let size=if cfg!(target_os="macos"){512}else{64};egui::IconData{rgba:crate::icon_art::icon_rgba(size),width:size,height:size}}
/// An event name that is still the placeholder of the language `from`, in the language `to`.
fn placeholder_in(event:&str,from:Lang,to:Lang)->Option<&'static str>{(event==lang::tr(from,FIRST_EVENT)).then(||lang::tr(to,FIRST_EVENT))}
/// The language switch: every language under its own name, so it can be found whatever language is on.
pub(crate) fn language_picker(ui:&mut Ui,picked:&mut Lang){
 ComboBox::from_id_salt("language").selected_text(format!("🌐  {}",picked.native_name())).width(104.0).show_ui(ui,|ui|{for l in Lang::ALL{ui.selectable_value(picked,l,l.native_name());}}).response.on_hover_text(t("언어"));
}
/// The look shared by the editor and the setup window: fonts, dark colours, spacing.
pub(crate) fn theme(ctx:&Context,language:Lang){
 install_fonts(ctx,language,false);
 let mut style=(*ctx.style()).clone();style.visuals=Visuals::dark();style.visuals.panel_fill=PANEL;style.visuals.window_fill=PANEL;style.visuals.override_text_color=Some(WHITE);style.visuals.selection.bg_fill=Color32::from_rgb(43,85,76);style.visuals.widgets.inactive.bg_fill=Color32::from_rgb(33,40,50);style.visuals.widgets.inactive.bg_stroke=Stroke::new(1.0_f32,LINE);style.visuals.widgets.hovered.bg_fill=Color32::from_rgb(48,60,70);style.visuals.widgets.active.bg_fill=Color32::from_rgb(49,80,73);style.spacing.item_spacing=vec2(9.0,10.0);style.spacing.button_padding=vec2(13.0,9.0);style.spacing.interact_size=vec2(30.0,34.0);style.text_styles.insert(TextStyle::Body,FontId::proportional(14.0));style.text_styles.insert(TextStyle::Button,FontId::proportional(14.0));style.text_styles.insert(TextStyle::Small,FontId::proportional(11.0));ctx.set_style_of(Theme::Dark,style);ctx.set_theme(ThemePreference::Dark);
}
/// Which face of a font file: its place in a collection (.ttc), or — where a collection's faces
/// do not stand in the same order on every version of the system — the upright face whose
/// PostScript name has this part in it and whose weight is nearest (400 regular, 700 bold).
#[derive(Clone,Copy,Debug,PartialEq)] enum Pick{Nth(u32),#[cfg_attr(not(target_os="macos"),allow(dead_code))] Like(&'static str,u16)}
use Pick::Nth;
/// A face of a font file: the file's name in Windows' font folder (on macOS its whole path) and which face of it.
type Face=(&'static str,Pick);
/// Windows' own fonts for each language, regular and bold; the first that is there is used.
/// Korean keeps Malgun Gothic, as before. English gets Segoe UI (the same letters, but a backslash
/// that is not drawn as the won sign), Chinese Microsoft YaHei UI and Japanese Yu Gothic UI (the
/// second face of their collections): Malgun Gothic has neither the simplified characters nor
/// Japan's forms.
#[cfg(not(target_os="macos"))]
fn language_faces(language:Lang)->(&'static [Face],&'static [Face]){match language{
 Lang::Ko=>(KOREAN_FACES.0,KOREAN_FACES.1),
 Lang::En=>(&[("segoeui.ttf",Nth(0))],&[("segoeuib.ttf",Nth(0))]),
 Lang::Zh=>(&[("msyh.ttc",Nth(1)),("msyh.ttc",Nth(0)),("msyh.ttf",Nth(0)),("simsun.ttc",Nth(0))],&[("msyhbd.ttc",Nth(1)),("msyhbd.ttc",Nth(0)),("msyhbd.ttf",Nth(0))]),
 Lang::Ja=>(&[("YuGothM.ttc",Nth(1)),("YuGothM.ttc",Nth(0)),("meiryo.ttc",Nth(2)),("meiryo.ttc",Nth(0)),("msgothic.ttc",Nth(1))],&[("YuGothB.ttc",Nth(1)),("YuGothB.ttc",Nth(0)),("meiryob.ttc",Nth(2)),("meiryob.ttc",Nth(0))]),
}}
#[cfg(not(target_os="macos"))]
const KOREAN_FACES:(&[Face],&[Face])=(&[("malgun.ttf",Nth(0))],&[("malgunbd.ttf",Nth(0))]);
/// For characters none of a language's fonts has: Microsoft YaHei covers Chinese and Japanese.
#[cfg(not(target_os="macos"))]
const WIDE_FACES:&[Face]=&[("msyh.ttc",Nth(1)),("msyh.ttc",Nth(0)),("msyh.ttf",Nth(0)),("simsun.ttc",Nth(0))];
/// Digits of the floating timer: Segoe UI Bold draws them cleanly.
#[cfg(not(target_os="macos"))]
const DIGIT_FACES:&[Face]=&[("segoeuib.ttf",Nth(0))];
/// macOS' own fonts for each language, regular and bold: Apple SD Gothic Neo for Korean, Helvetica
/// for English, Hiragino Sans for Japanese, and for Chinese PingFang SC where macOS keeps it with
/// the other fonts (older versions do; macOS 26 does not), else Hiragino Sans GB, which every
/// version has. Their collections hold many weights, and some the forms of several regions, so a
/// face is found by its name and weight.
#[cfg(target_os="macos")]
fn language_faces(language:Lang)->(&'static [Face],&'static [Face]){use Pick::Like;match language{
 Lang::Ko=>(KOREAN_FACES.0,KOREAN_FACES.1),
 Lang::En=>(&[("/System/Library/Fonts/Helvetica.ttc",Like("Helvetica",400)),("/System/Library/Fonts/Supplemental/Arial.ttf",Nth(0))],&[("/System/Library/Fonts/Helvetica.ttc",Like("Helvetica",700)),("/System/Library/Fonts/Supplemental/Arial Bold.ttf",Nth(0))]),
 Lang::Zh=>(&[("/System/Library/Fonts/PingFang.ttc",Like("PingFangSC-",400)),("/System/Library/Fonts/Hiragino Sans GB.ttc",Like("HiraginoSansGB-",300)),("/System/Library/Fonts/STHeiti Medium.ttc",Like("STHeitiSC-",400)),("/System/Library/Fonts/Supplemental/Arial Unicode.ttf",Nth(0))],
  &[("/System/Library/Fonts/PingFang.ttc",Like("PingFangSC-",600)),("/System/Library/Fonts/Hiragino Sans GB.ttc",Like("HiraginoSansGB-",600)),("/System/Library/Fonts/STHeiti Medium.ttc",Like("STHeitiSC-",400))]),
 Lang::Ja=>(&[("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",Like("HiraginoSans-",300)),("/System/Library/Fonts/Hiragino Sans GB.ttc",Like("HiraginoSansGB-",300)),("/System/Library/Fonts/Supplemental/Arial Unicode.ttf",Nth(0))],
  &[("/System/Library/Fonts/ヒラギノ角ゴシック W6.ttc",Like("HiraginoSans-",600)),("/System/Library/Fonts/ヒラギノ角ゴシック W7.ttc",Like("HiraginoSans-",700)),("/System/Library/Fonts/Hiragino Sans GB.ttc",Like("HiraginoSansGB-",600))]),
}}
#[cfg(target_os="macos")]
const KOREAN_FACES:(&[Face],&[Face])=(&[("/System/Library/Fonts/AppleSDGothicNeo.ttc",Pick::Like("AppleSDGothicNeo-",400)),("/System/Library/Fonts/Supplemental/AppleGothic.ttf",Nth(0))],&[("/System/Library/Fonts/AppleSDGothicNeo.ttc",Pick::Like("AppleSDGothicNeo-",700))]);
/// For characters none of a language's fonts has: Arial Unicode covers Korean, Chinese and Japanese.
#[cfg(target_os="macos")]
const WIDE_FACES:&[Face]=&[("/System/Library/Fonts/Supplemental/Arial Unicode.ttf",Nth(0)),("/System/Library/Fonts/PingFang.ttc",Pick::Like("PingFangSC-",400)),("/System/Library/Fonts/Hiragino Sans GB.ttc",Pick::Like("HiraginoSansGB-",300))];
/// Digits of the floating timer: Helvetica Bold draws them cleanly.
#[cfg(target_os="macos")]
const DIGIT_FACES:&[Face]=&[("/System/Library/Fonts/Helvetica.ttc",Pick::Like("Helvetica",700)),("/System/Library/Fonts/Supplemental/Arial Bold.ttf",Nth(0))];
/// The face of a font file's bytes that `Pick::Like` asks for: among the upright faces whose
/// PostScript name has `part` in it, the one nearest in weight, and of equals the first.
fn face_like(bytes:&[u8],part:&str,weight:u16)->Option<u32>{
 (0..ttf_parser::fonts_in_collection(bytes).unwrap_or(1)).filter_map(|index|{
  let face=ttf_parser::Face::parse(bytes,index).ok()?;if face.is_italic()||face.is_oblique(){return None;}
  let named=part.is_empty()||postscript_names(&face).iter().any(|name|name.contains(part));
  named.then(||((face.weight().to_number() as i32-weight as i32).abs(),index))
 }).min().map(|(_,index)|index)
}
/// A face's PostScript names. They are plain letters; a font may keep them as two-byte text (which
/// ttf-parser reads) or, in the old Macintosh way, one byte a letter.
fn postscript_names(face:&ttf_parser::Face)->Vec<String>{face.names().into_iter().filter(|name|name.name_id==ttf_parser::name_id::POST_SCRIPT_NAME).map(|name|name.to_string().unwrap_or_else(||String::from_utf8_lossy(name.name).into_owned())).collect()}
/// A font of the system (Windows' font folder; on macOS the file's whole path is given). A file is
/// read once and kept for as long as the program runs, so switching languages back and forth
/// neither reads nor holds it again.
fn system_font(face:Face)->Option<FontData>{
 static READ:std::sync::OnceLock<std::sync::Mutex<HashMap<&'static str,Option<&'static [u8]>>>>=std::sync::OnceLock::new();
 let(file,pick)=face;
 let bytes=(*READ.get_or_init(Default::default).lock().ok()?.entry(file).or_insert_with(||{
  // (Joining a whole path leaves it as it is.)
  let folder=std::env::var_os("WINDIR").map_or_else(||PathBuf::from("C:/Windows"),PathBuf::from).join("Fonts");
  std::fs::read(folder.join(file)).ok().map(|data|&*Box::leak(data.into_boxed_slice()))
 }))?;
 // egui stops the program on data it cannot read as a font, so only a file that starts like
 // one, with that many faces in it and that face readable, is handed over.
 let faces=match bytes.get(..4)?{b"ttcf"=>u32::from_be_bytes(bytes.get(8..12)?.try_into().ok()?),[0,1,0,0]|b"OTTO"|b"true"=>1,_=>0};
 let index=match pick{Nth(index)=>index,Pick::Like(part,weight)=>face_like(bytes,part,weight)?};
 if index>=faces||ttf_parser::Face::parse(bytes,index).is_err(){return None;}
 let mut data=FontData::from_static(bytes);data.index=index;Some(data)
}
/// The fonts of a language: its own first, the Korean ones behind them for Korean in a brand or
/// event name, and a bold family for the floating timer. `wide` adds one more font for
/// characters none of them has.
pub(crate) fn install_fonts(ctx:&Context,language:Lang,wide:bool){
 let mut fonts=FontDefinitions::default();let defaults=fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
 let(mut body,mut heavy)=(Vec::<String>::new(),Vec::<String>::new());
 let mut add=|list:&mut Vec<String>,name:&str,faces:&[Face]|{
  if !fonts.font_data.contains_key(name){if let Some(data)=faces.iter().find_map(|f|system_font(*f)){fonts.font_data.insert(name.into(),data.into());}}
  if fonts.font_data.contains_key(name)&&!list.iter().any(|n|n==name){list.push(name.into());}
 };
 add(&mut heavy,"DigitsBold",DIGIT_FACES);
 // Korean's own fonts are the ones every language falls back on, so they are listed once.
 let(regular,bold_faces)=language_faces(language);let own=language!=Lang::Ko;
 if own{add(&mut body,"Language",regular);add(&mut heavy,"LanguageBold",bold_faces);}
 add(&mut body,"Korean",KOREAN_FACES.0);add(&mut heavy,"KoreanBold",KOREAN_FACES.1);
 if own{add(&mut heavy,"Language",regular);}add(&mut heavy,"Korean",KOREAN_FACES.0);
 if wide{add(&mut body,"Wide",WIDE_FACES);add(&mut heavy,"Wide",WIDE_FACES);}
 body.extend(defaults.iter().cloned());heavy.extend(defaults);
 fonts.families.insert(FontFamily::Proportional,body);fonts.families.insert(bold(),heavy);ctx.set_fonts(fonts);
}
fn field_label(ui:&mut Ui,s:&str){ui.label(RichText::new(s).size(12.0).color(MUTED));}
fn duration_editor(ui:&mut Ui,v:&mut f64,id:&str){ui.push_id(id,|ui|{ui.horizontal(|ui|{let mut m=(*v/60.0).floor() as u32;let mut s=(*v as u32)%60;let a=ui.add(DragValue::new(&mut m).range(0..=5999).suffix(t(" 분"))).changed();let b=ui.add(DragValue::new(&mut s).range(0..=59).suffix(t(" 초"))).changed();if a||b{*v=((m*60+s) as f64).min(timer::MAX_SECONDS);}});});}
fn play_sound(volume:f32,path:Option<PathBuf>,end:bool){std::thread::spawn(move||{use rodio::Source;if let Ok((_stream,handle))=rodio::OutputStream::try_default(){if let Ok(sink)=rodio::Sink::try_new(&handle){sink.set_volume(volume);if let Some(path)=path{if let Ok(f)=std::fs::File::open(path){if let Ok(src)=rodio::Decoder::new(std::io::BufReader::new(f)){sink.append(src);sink.sleep_until_end();return;}}}for hz in if end{vec![659.25,783.99,1046.5]}else{vec![880.0,880.0]}{sink.append(rodio::source::SineWave::new(hz).take_duration(Duration::from_millis(180)).amplify(0.22));}sink.sleep_until_end();}}});}
impl eframe::App for Studio{
 fn update(&mut self,ctx:&Context,frame:&mut eframe::Frame){
  self.frames+=1;if self.hwnd.is_none(){self.hwnd=win::handle(frame);}
  lang::set(self.s.language);self.font_check(ctx);if self.frames==40{if let Some(l)=self.auto_language{self.set_language(ctx,l);}}
  for ev in ctx.input(|i|i.events.clone()){if let Event::Screenshot{image,..}=ev{if let Some(path)=&self.shot{if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}let bytes:Vec<u8>=image.pixels.iter().flat_map(|p|p.to_srgba_unmultiplied()).collect();match image::save_buffer(path,&bytes,image.size[0] as u32,image.size[1] as u32,image::ColorType::Rgba8){Ok(())=>{self.screenshot_ok=true;ctx.send_viewport_cmd(ViewportCommand::Close);},Err(e)=>{eprintln!("Screenshot failed: {e}");ctx.send_viewport_cmd(ViewportCommand::Close);}}}}}
  if ctx.input(|i|i.pointer.delta()!=Vec2::ZERO||i.pointer.any_pressed()){self.activity=Instant::now();}
  self.hotkey_sync(ctx);self.window_upkeep(ctx);if self.auto_float.is_some_and(|at|self.frames==at){self.enter_floating(ctx);}if self.auto_back.is_some_and(|at|self.frames==at){self.leave_floating(ctx);}
  self.keys(ctx);if self.s.qna_enabled{self.clock.auto_qna(self.s.qna_duration,self.now());}let phase=self.clock.view(self.now(),self.s.warning,self.s.count_up,self.s.overtime,self.s.seconds_only).phase;if phase!=self.prior_phase{if phase==Phase::Warning{self.sound(false);}if phase==Phase::End||(phase==Phase::Over&&self.prior_phase!=Phase::End){self.sound(true);self.flash_since=Some(Instant::now());}self.prior_phase=phase;}
  if self.overlay{self.floating(ctx);}
  else if self.presentation{CentralPanel::default().frame(Frame::new().fill(BG)).show(ctx,|ui|{let r=ui.max_rect();let v=self.clock.view(self.now(),self.s.warning,self.s.count_up,self.s.overtime,self.s.seconds_only);let tr=scene::render(ui.painter(),r,&self.s,&v,&self.textures,self.clock.qna,self.clock.running());ui.put(Rect::from_min_size(r.left_top()+vec2(12.0,12.0),vec2(140.0,28.0)),Label::new(RichText::new(t("Esc  편집 화면으로")).color(MUTED).size(11.0)));if let Some(cmd)=self.hover_bar(ui,"presentation",r,tr,self.activity){self.apply_cmd(cmd,ui.ctx());}});if !self.force_controls&&self.activity.elapsed().as_secs_f64()>=CONTROLS_IDLE{ctx.set_cursor_icon(CursorIcon::None);}}
  else{self.top(ctx);self.controls(ctx);self.inspector(ctx);self.workspace(ctx);self.packer_window(ctx);}
  if self.confirm_reset&&!self.overlay{Window::new(t("타이머 초기화")).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui|{ui.label(t("진행 시간을 지우고 설정한 발표 시간으로 돌아갈까요?"));ui.horizontal(|ui|{if ui.button(t("초기화")).clicked(){self.reset();}if ui.button(t("취소")).clicked(){self.confirm_reset=false;}});});}
  if self.audience{ctx.show_viewport_immediate(ViewportId::from_hash_of("audience"),ViewportBuilder::default().with_title(t("행사 발표 화면 · F11 전체 화면")).with_inner_size([1100.0,720.0]).with_min_inner_size([480.0,320.0]),|ctx,_|{if ctx.input(|i|i.viewport().close_requested()){self.audience=false;}if fullscreen_key(ctx){self.audience_full=!self.audience_full;self.settle();ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.audience_full));}if ctx.input(|i|i.key_pressed(Key::Escape)){self.audience_full=false;self.settle();ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));}if ctx.input(|i|i.key_pressed(Key::Space)){self.toggle();}
   if ctx.input(|i|i.pointer.delta()!=Vec2::ZERO||i.pointer.any_pressed()){self.audience_activity=Instant::now();}
   CentralPanel::default().frame(Frame::new().fill(BG)).show(ctx,|ui|{let v=self.clock.view(self.now(),self.s.warning,self.s.count_up,self.s.overtime,self.s.seconds_only);let r=ui.max_rect();let tr=scene::render(ui.painter(),r,&self.s,&v,&self.textures,self.clock.qna,self.clock.running());if let Some(cmd)=self.hover_bar(ui,"audience",r,tr,self.audience_activity){self.apply_cmd(cmd,ui.ctx());}});
   if !self.force_controls&&ctx.input(|i|i.pointer.hover_pos().is_some())&&self.audience_activity.elapsed().as_secs_f64()>=CONTROLS_IDLE{ctx.set_cursor_icon(CursorIcon::None);}
   if let Some(wait)=self.next_frame_in(){ctx.request_repaint_after(wait);}});}
  let serial=serde_json::to_string(&self.s).unwrap();if serial!=self.last_saved&&self.shot.is_none(){if self.dirty_since.is_none(){self.dirty_since=Some(Instant::now());}if self.dirty_since.is_some_and(|t|t.elapsed()>SAVE_AFTER){self.save();
   // (The status line of this frame is already drawn: once more, to show how the save went.)
   ctx.request_repaint();}}
  // Changed and changed back: nothing is waiting to be saved any more.
  else{self.dirty_since=None;}
  if self.shot.is_some()&&!self.shot_requested&&self.frames>=120{ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));self.shot_requested=true;}if self.shot.is_some()&&self.frames>300&&!self.screenshot_ok{eprintln!("Screenshot timeout");ctx.send_viewport_cmd(ViewportCommand::Close);}
  if let Some(wait)=self.next_frame_in(){ctx.request_repaint_after(wait);}
 }
 /// macOS tells a window where the pointer is only while its app is in front and the window takes
 /// the mouse, and the floating timer is mostly neither. So the pointer is followed by hand there
 /// (win_mac.rs `follow_pointer`, which also lets clicks through the empty parts) and given to
 /// egui here as if the window had been told: the hover buttons then appear as on Windows.
 fn raw_input_hook(&mut self,ctx:&Context,raw:&mut RawInput){
  if !win::FOLLOWS_POINTER||!self.overlay||raw.viewport_id!=ViewportId::ROOT{return;}let Some(h)=self.hwnd else{return};
  let at=win::follow_pointer(h);if at==self.pointer_at{return;}self.pointer_at=at;
  match at{Some((x,y))=>{let k=win::units_per_point(ctx);raw.events.push(Event::PointerMoved(pos2(x as f32/k,y as f32/k)));},None if self.gesture.is_none()=>raw.events.push(Event::PointerGone),None=>{}}
 }
 /// Clear to see-through: the editor's panels cover every pixel, the floating timer leaves the rest empty.
 fn clear_color(&self,_visuals:&egui::Visuals)->[f32;4]{[0.0;4]}
 fn on_exit(&mut self,_gl:Option<&eframe::glow::Context>){if self.shot.is_none()&&serde_json::to_string(&self.s).unwrap()!=self.last_saved{self.save();}}
}
pub fn run()->eframe::Result{let args:Vec<String>=std::env::args().collect();let shot=args.iter().position(|x|x=="--screenshot").and_then(|i|args.get(i+1)).map(PathBuf::from);let demo=args.iter().position(|x|x=="--demo").and_then(|i|args.get(i+1)).cloned();
 // `--overlay`, or the saved choice (never for a check's `--screenshot`, which shows the editor).
 let(saved,_)=load_settings(&app_root());let floating=args.iter().any(|x|x=="--overlay")||(saved.overlay_start&&shot.is_none());
 // `--lang ko|en|zh|ja` opens in that language whatever is saved.
 let language=args.iter().position(|x|x=="--lang").and_then(|i|args.get(i+1)).and_then(|code|Lang::from_code(code));lang::set(language.unwrap_or(saved.language));
 // `--demo narrow`: the editor at its smallest size, to check that every language's top bar fits.
 let editor=if demo.as_deref().is_some_and(|d|d.split(',').any(|x|x=="narrow")){EDITOR_MIN}else{EDITOR_SIZE};
 let viewport=ViewportBuilder::default().with_transparent(true).with_icon(window_icon());let viewport=if floating{viewport.with_title(t(FLOAT_TITLE)).with_decorations(false).with_always_on_top().with_inner_size(FLOAT_SIZE).with_min_inner_size(FLOAT_MIN)}else{viewport.with_title(t(EDITOR_TITLE)).with_inner_size(editor).with_min_inner_size(EDITOR_MIN)};
 // A check that asks for a fixed state (`--demo`) takes its picture without taking the keyboard from whoever is at the computer.
 let viewport=if shot.is_some()&&demo.is_some(){viewport.with_active(false)}else{viewport};
 // `--demo hidden`: the check's window is put beyond the edge of the screen, so nothing flashes up in front of the person.
 let viewport=if shot.is_some()&&demo.as_deref().is_some_and(|d|d.split(',').any(|x|x=="hidden")){viewport.with_position(OUT_OF_SIGHT)}else{viewport};
 let options=eframe::NativeOptions{viewport,renderer:eframe::Renderer::Glow,..Default::default()};eframe::run_native("Event Studio",options,Box::new(move|cc|Ok(Box::new(Studio::new(cc,shot,demo,floating,language)))))}
