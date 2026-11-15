use super::*;
fn fixture(name:&str)->PathBuf{let p=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("checks").join(name);std::fs::create_dir_all(&p).unwrap();p}
#[test] fn settings_roundtrip(){let mut s=Settings::default();s.brand="테스트 브랜드".into();s.event="한글 행사명 테스트".into();s.logo="assets/logo.png".into();s.mode=Mode::Number;s.qna_enabled=true;let text=serde_json::to_string(&s).unwrap();let r:Settings=serde_json::from_str(&text).unwrap();assert_eq!(r.event,s.event);assert_eq!(r.logo,s.logo);assert_eq!(r.mode,Mode::Number);assert!(r.qna_enabled);}
#[test] fn settings_validation(){let mut s:Settings=serde_json::from_str(r#"{"duration":-4,"warning":999999,"logo_position":88,"volume":3.0,"dim":-1.0}"#).unwrap();s.normalize();assert_eq!(s.duration,0.0);assert_eq!(s.warning,timer::MAX_SECONDS);assert_eq!(s.logo_position,5);assert_eq!(s.volume,1.0);assert_eq!(s.dim,0.0);assert_eq!(s.qna_duration,300.0);}
#[test] fn image_load_transparency(){let root=fixture("image-test");let path=root.join("한글 로고.png");let mut image=image::RgbaImage::new(120,60);image.put_pixel(20,20,image::Rgba([132,226,190,128]));image.save(&path).unwrap();let decoded=load_image(&path).unwrap();assert_eq!(decoded.dimensions(),(120,60));assert_eq!(decoded.get_pixel(20,20).0,[132,226,190,128]);}
#[test] fn invalid_image_rejected(){let root=fixture("image-test");let path=root.join("not-an-image.png");std::fs::write(&path,b"not an image").unwrap();assert!(load_image(&path).is_err());}
#[test] fn settings_atomic_save_and_backup(){let root=fixture("settings-test");let mut app=Studio::bare(Settings::default(),root.clone(),None);app.s.brand="first".into();app.save();assert_eq!(serde_json::from_str::<Settings>(&std::fs::read_to_string(root.join("settings.json")).unwrap()).unwrap().brand,"first");app.s.brand="second".into();app.save();assert_eq!(serde_json::from_str::<Settings>(&std::fs::read_to_string(root.join("settings.backup.json")).unwrap()).unwrap().brand,"first");assert_eq!(serde_json::from_str::<Settings>(&std::fs::read_to_string(root.join("settings.json")).unwrap()).unwrap().brand,"second");assert!(!root.join("settings.pending.json").exists());}
#[test] fn older_settings_get_floating_defaults(){let s:Settings=serde_json::from_str(r#"{"brand":"B","mode":"Bar"}"#).unwrap();assert!(!s.overlay_start&&s.overlay_top_right&&!s.overlay_scene&&!s.hotkeys);assert_eq!(s.overlay_backdrop,0.0);assert!(s.overlay_rect.is_none());}
#[test] fn floating_settings_limits(){let mut s:Settings=serde_json::from_str(r#"{"overlay_backdrop":3.0,"overlay_rect":[10,10,0,5]}"#).unwrap();s.normalize();assert_eq!(s.overlay_backdrop,BACKDROP_MAX);assert!(s.overlay_rect.is_none());let mut s:Settings=serde_json::from_str(r#"{"overlay_backdrop":-1.0,"overlay_rect":[10,10,360,200]}"#).unwrap();s.normalize();assert_eq!(s.overlay_backdrop,0.0);assert_eq!(s.overlay_rect,Some([10,10,360,200]));}
#[test] fn floating_status_lines(){let mut c=Clock::new(300.0);let v=c.view(0.0,60.0,false,true,false);assert_eq!(overlay::status_text(&v,false,false,false,false,300.0),"");assert_eq!(overlay::status_text(&v,true,false,false,false,300.0),"목표 05:00");c.toggle(0.0);let v=c.view(250.0,60.0,false,true,false);assert_eq!(overlay::status_text(&v,false,true,true,false,300.0),"곧 종료");let v=c.view(300.5,60.0,false,true,false);assert_eq!(overlay::status_text(&v,false,true,true,false,300.0),"시간 종료");let v=c.view(302.0,60.0,false,true,false);assert_eq!(overlay::status_text(&v,false,true,true,false,300.0),"시간 초과");c.toggle(100.0);let v=c.view(100.0,60.0,false,true,false);assert_eq!(overlay::status_text(&v,false,false,true,false,300.0),"일시정지");}
// The floating timer driven by simulated pointer events (never the real mouse).
fn float_frame(ctx:&Context,app:&mut Studio,events:Vec<Event>){let _=ctx.run(RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,FLOAT_SIZE)),events,..Default::default()},|ctx|app.floating(ctx));}
fn float_click(ctx:&Context,app:&mut Studio,at:Pos2){float_frame(ctx,app,vec![Event::PointerMoved(at)]);for pressed in [true,false]{float_frame(ctx,app,vec![Event::PointerButton{pos:at,button:PointerButton::Primary,pressed,modifiers:Modifiers::NONE}]);}}
fn float_press(ctx:&Context,app:&mut Studio,cmd:controls::Cmd){let at=app.float_buttons.iter().find(|b|b.0==cmd).map(|b|b.1.center()).expect("button shown");float_click(ctx,app,at);}
#[test] fn floating_timer_shows_buttons_on_hover_and_they_work(){
 let ctx=Context::default();install_fonts(&ctx,Lang::Ko,false);let mut app=Studio::bare(Settings::default(),fixture("float-test"),None);app.overlay=true;
 for _ in 0..5{float_frame(&ctx,&mut app,vec![]);}assert!(app.float_buttons.is_empty(),"no buttons until the mouse is over it");
 for i in 0..30{float_frame(&ctx,&mut app,vec![Event::PointerMoved(pos2(120.0+i as f32,90.0))]);}
 assert_eq!(app.float_buttons.iter().map(|b|b.0).collect::<Vec<_>>(),[controls::Cmd::Reset,controls::Cmd::Play,controls::Cmd::Pause,controls::Cmd::Editor,controls::Cmd::Quit]);
 assert!(app.float_buttons.iter().all(|b|Rect::from_min_size(Pos2::ZERO,FLOAT_SIZE).contains_rect(b.1)),"buttons stay inside the window");
 float_press(&ctx,&mut app,controls::Cmd::Play);assert!(app.clock.running(),"재시작/시작 starts");
 float_press(&ctx,&mut app,controls::Cmd::Pause);assert!(!app.clock.running(),"멈춤 pauses");
 float_press(&ctx,&mut app,controls::Cmd::Play);assert!(app.clock.running(),"재시작 resumes");
 float_press(&ctx,&mut app,controls::Cmd::Reset);assert!(!app.clock.running()&&app.clock.elapsed(app.now())==0.0,"리셋 goes back to the full time, stopped");
 float_press(&ctx,&mut app,controls::Cmd::Editor);assert!(!app.overlay,"the gear goes back to the editor");
}
#[test] fn floating_buttons_hide_when_the_mouse_leaves(){
 let ctx=Context::default();install_fonts(&ctx,Lang::Ko,false);let mut app=Studio::bare(Settings::default(),fixture("float-test"),None);app.overlay=true;
 for i in 0..30{float_frame(&ctx,&mut app,vec![Event::PointerMoved(pos2(60.0+i as f32,60.0))]);}assert!(!app.float_buttons.is_empty());
 float_frame(&ctx,&mut app,vec![Event::PointerGone]);for _ in 0..30{float_frame(&ctx,&mut app,vec![]);}assert!(app.float_buttons.is_empty());
}
#[test] fn floating_timer_tiny_window_keeps_buttons_inside(){
 let ctx=Context::default();install_fonts(&ctx,Lang::Ko,false);let mut app=Studio::bare(Settings::default(),fixture("float-test"),None);app.overlay=true;app.force_controls=true;
 for _ in 0..30{let _=ctx.run(RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,FLOAT_MIN)),..Default::default()},|ctx|app.floating(ctx));}
 assert_eq!(app.float_buttons.len(),5);assert!(app.float_buttons.iter().all(|b|Rect::from_min_size(Pos2::ZERO,FLOAT_MIN).expand(1.0).contains_rect(b.1)));
}
fn painted_parts(mode:Mode,backdrop:f32)->Vec<overlay::Solid>{let ctx=Context::default();install_fonts(&ctx,Lang::Ko,false);let mut s=Settings::default();s.mode=mode;s.overlay_backdrop=backdrop;let v=Clock::new(300.0).view(0.0,60.0,false,true,false);let rect=Rect::from_min_size(Pos2::ZERO,FLOAT_SIZE);let mut solid=Vec::new();let _=ctx.run(RawInput{screen_rect:Some(rect),..Default::default()},|ctx|{CentralPanel::default().frame(Frame::NONE).show(ctx,|ui|{solid=overlay::paint(ui.painter(),rect,&s,&v,"",&HashMap::new());});});solid}
// What the floating timer paints in a window of `size` with these settings.
fn painted_with(s:&Settings,size:Vec2)->Vec<overlay::Solid>{let ctx=Context::default();install_fonts(&ctx,Lang::Ko,false);let v=Clock::new(300.0).view(0.0,60.0,false,true,false);let rect=Rect::from_min_size(Pos2::ZERO,size);let mut solid=Vec::new();let _=ctx.run(RawInput{screen_rect:Some(rect),..Default::default()},|ctx|{CentralPanel::default().frame(Frame::NONE).show(ctx,|ui|{solid=overlay::paint(ui.painter(),rect,s,&v,"",&HashMap::new());});});solid}
fn ring_of(solid:&[overlay::Solid])->(Pos2,f32){solid.iter().find_map(|a|match a{overlay::Solid::Disc(c,r)=>Some((*c,*r)),_=>None}).expect("a ring")}
// "배경이 투명하게": with brand, event name and logos shown, the floating timer still has no background.
#[test] fn floating_timer_with_titles_keeps_the_background_see_through(){
 let size=vec2(375.0,495.0);let mut s=Settings::default();s.brand="브랜드".into();s.event="행사 이름".into();s.subtitle="세션".into();
 let plain=painted_with(&s,size);s.overlay_scene=true;let titled=painted_with(&s,size);
 for corner in [pos2(4.0,4.0),pos2(371.0,4.0),pos2(4.0,491.0),pos2(371.0,491.0),pos2(8.0,247.0)]{assert!(!titled.iter().any(|a|a.contains(corner)),"nothing is painted at {corner:?}");}
 assert!(!titled.iter().any(|a|matches!(a,overlay::Solid::Rect(r) if r.width()>=size.x-1.0&&r.height()>=size.y-1.0)),"no sheet behind it all");
 assert_eq!(titled.len(),plain.len()+3,"brand, event name and subtitle are painted");
 let(plain_ring,titled_ring)=(ring_of(&plain),ring_of(&titled));assert!(titled_ring.0.y>plain_ring.0.y,"the ring moves down to make room");
 let titles_bottom=titled.iter().filter_map(|a|match a{overlay::Solid::Rect(r)=>Some(r.bottom()),_=>None}).fold(0.0,f32::max);assert!(titles_bottom<=titled_ring.0.y-titled_ring.1+8.0,"titles end where the ring starts");assert!(titles_bottom<=size.y*0.5);
 // Nothing to show: the same as without the option.
 s.brand.clear();s.event.clear();s.subtitle="  ".into();assert_eq!(painted_with(&s,size),plain);
 // A backdrop is the only thing that fills the window, and only when asked for.
 s.overlay_backdrop=0.3;assert!(painted_with(&s,size).iter().any(|a|a.contains(pos2(4.0,4.0))));
}
#[test] fn settings_from_before_the_see_through_stage_start_see_through(){
 let mut s:Settings=serde_json::from_str(r#"{"overlay_scene":true,"overlay_backdrop":0.07}"#).unwrap();s.normalize();s.migrate();assert_eq!(s.overlay_backdrop,0.0);assert_eq!(s.schema,SCHEMA);assert!(s.overlay_scene);
 let mut s:Settings=serde_json::from_str(r#"{"schema":2,"overlay_scene":true,"overlay_backdrop":0.25}"#).unwrap();s.normalize();s.migrate();assert_eq!(s.overlay_backdrop,0.25);
 let mut s:Settings=serde_json::from_str(r#"{"overlay_scene":false,"overlay_backdrop":0.25}"#).unwrap();s.normalize();s.migrate();assert_eq!(s.overlay_backdrop,0.25);
 let again:Settings=serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();assert_eq!(again.schema,SCHEMA);
}
// As on a per-pixel-alpha layered window, only painted pixels take the mouse.
#[test] fn floating_timer_only_painted_parts_take_the_mouse(){
 let centre=pos2(FLOAT_SIZE.x/2.0,FLOAT_SIZE.y/2.0);let corner=pos2(6.0,6.0);
 for mode in [Mode::Circle,Mode::Bar,Mode::Number]{let solid=painted_parts(mode.clone(),0.0);assert!(solid.iter().any(|a|a.contains(centre)),"{mode:?}: the time takes the mouse");assert!(!solid.iter().any(|a|a.contains(corner)),"{mode:?}: an empty corner lets it through");}
 let solid=painted_parts(Mode::Circle,0.4);assert!(solid.iter().any(|a|a.contains(corner)),"a backdrop is paint too");
 let px=overlay::Solid::Disc(pos2(100.0,50.0),40.0).to_px(1.5);assert_eq!(px,win::HitShape::Ellipse(win::PxRect{x:85,y:10,w:130,h:130}));
}
#[test] fn hover_buttons_drive_the_clock(){let mut app=Studio::bare(Settings::default(),fixture("hover-test"),None);let ctx=Context::default();app.apply_cmd(controls::Cmd::Pause,&ctx);assert!(!app.clock.running());app.apply_cmd(controls::Cmd::Play,&ctx);assert!(app.clock.running());app.apply_cmd(controls::Cmd::Play,&ctx);assert!(app.clock.running());app.apply_cmd(controls::Cmd::Pause,&ctx);assert!(!app.clock.running());app.clock.accumulated=42.0;app.flash_since=Some(Instant::now());app.apply_cmd(controls::Cmd::Reset,&ctx);assert!(!app.clock.running()&&app.clock.elapsed(app.now())==0.0&&app.flash_since.is_none());}
// Languages: the saved choice, the first start, and whether each language's fonts can draw its texts.
#[test] fn language_is_saved_and_older_settings_stay_korean(){
 let old:Settings=serde_json::from_str(r#"{"brand":"B","mode":"Bar"}"#).unwrap();assert_eq!(old.language,Lang::Ko);
 let mut s=Settings::default();assert_eq!(s.language,Lang::Ko);s.language=Lang::Ja;let text=serde_json::to_string(&s).unwrap();assert!(text.contains(r#""language":"ja""#));assert_eq!(serde_json::from_str::<Settings>(&text).unwrap().language,Lang::Ja);
 let odd:Settings=serde_json::from_str(r#"{"language":"xx","brand":"B"}"#).unwrap();assert_eq!((odd.language,odd.brand.as_str()),(Lang::Ko,"B"));
 let first=Settings::first(Lang::Zh);assert_eq!((first.language,first.event.as_str(),first.duration),(Lang::Zh,"开启新可能的时刻",timer::DEFAULT_SECONDS));
 assert_eq!(Settings::first(Lang::Ko).event,Settings::default().event);
}
#[test] fn switching_language_keeps_what_the_person_typed(){
 // The placeholder event name follows the language; a typed one stays. (Checked without switching the
 // language itself: that is one switch for the whole program, and the other tests read Korean.)
 assert_eq!(placeholder_in(FIRST_EVENT,Lang::Ko,Lang::En),Some("A time that opens new possibilities"));
 assert_eq!(placeholder_in("A time that opens new possibilities",Lang::En,Lang::Ja),Some("新しい可能性をひらく時間"));
 assert_eq!(placeholder_in("新しい可能性をひらく時間",Lang::Ja,Lang::Ko),Some(FIRST_EVENT));
 assert_eq!(placeholder_in("우리 행사",Lang::Ko,Lang::En),None);
 assert_eq!(placeholder_in(FIRST_EVENT,Lang::En,Lang::Ja),None,"Korean wording typed while English is on is the person's own text");
}
#[test] fn every_language_can_draw_its_own_texts(){
 for language in Lang::ALL{
  if language_faces(language).0.iter().all(|f|system_font(*f).is_none()){eprintln!("{language:?}: its fonts are not on this computer, not checked");continue;}
  let ctx=Context::default();install_fonts(&ctx,language,false);
  let _=ctx.run(RawInput::default(),|ctx|ctx.fonts(|f|{
   for family in [FontFamily::Proportional,bold()]{let font=FontId::new(14.0,family);
    for row in crate::strings::TABLE{let text=lang::tr(language,row[0]);let missing:String=text.chars().filter(|c|!c.is_control()&&!f.has_glyphs(&font,&c.to_string())).collect();assert!(missing.is_empty(),"{language:?}: no glyph for {missing:?} in {text:?}");}
    // The language switch shows every language under its own name.
    for l in Lang::ALL{assert!(f.has_glyphs(&font,l.native_name()),"{language:?} cannot draw {}",l.native_name());}
   }
   assert!(f.has_glyphs(&FontId::proportional(14.0),"🌐"));
  }));
 }
}
#[test] fn a_missing_or_odd_font_file_is_passed_over(){
 assert!(system_font(("no-such-font.ttf",0)).is_none());
 // A face a file does not have: the single font file of Malgun Gothic has only face 0.
 if system_font(("malgun.ttf",0)).is_some(){assert!(system_font(("malgun.ttf",1)).is_none());}
 if let Some(collection)=system_font(("msyh.ttc",1)){assert_eq!(collection.index,1);assert!(system_font(("msyh.ttc",9)).is_none());}
}
/// One frame of only the language box, as the editor and the setup window show it.
fn picker_frame(ctx:&Context,picked:&mut Lang,events:Vec<Event>)->FullOutput{ctx.run(RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,vec2(420.0,320.0))),events,..Default::default()},|ctx|{CentralPanel::default().show(ctx,|ui|language_picker(ui,picked));})}
/// A click, and the frame after things have settled.
fn picker_click(ctx:&Context,picked:&mut Lang,at:Pos2)->FullOutput{picker_frame(ctx,picked,vec![Event::PointerMoved(at)]);for pressed in [true,false]{picker_frame(ctx,picked,vec![Event::PointerButton{pos:at,button:PointerButton::Primary,pressed,modifiers:Modifiers::NONE}]);}picker_frame(ctx,picked,vec![]);picker_frame(ctx,picked,vec![])}
/// The middle of a control, from what egui tells a screen reader about the frame: the test presses what is really drawn.
fn spot(frame:&FullOutput,is:impl Fn(&egui::accesskit::Node)->bool)->Option<Pos2>{frame.platform_output.accesskit_update.as_ref()?.nodes.iter().find(|(_,node)|is(node)).and_then(|(_,node)|node.bounds()).map(|b|pos2(((b.x0+b.x1)/2.0) as f32,((b.y0+b.y1)/2.0) as f32))}
#[test] fn the_language_box_lists_every_language_and_a_click_picks_it(){
 let ctx=Context::default();ctx.enable_accesskit();install_fonts(&ctx,Lang::Ko,false);let mut picked=Lang::Ko;
 for target in [Lang::Ja,Lang::En,Lang::Zh,Lang::Ko]{
  picker_frame(&ctx,&mut picked,vec![]);let closed=picker_frame(&ctx,&mut picked,vec![]);
  assert!(Lang::ALL.iter().all(|l|spot(&closed,|n|n.label()==Some(l.native_name())).is_none()),"the list is shut until the box is pressed");
  let the_box=spot(&closed,|n|n.role()==egui::accesskit::Role::ComboBox).expect("the language box is drawn");
  let open=picker_click(&ctx,&mut picked,the_box);
  let rows:Vec<Pos2>=Lang::ALL.iter().map(|l|spot(&open,|n|n.label()==Some(l.native_name())).unwrap_or_else(||panic!("{} is not in the list",l.native_name()))).collect();
  assert!(rows.windows(2).all(|w|w[0].y<w[1].y),"한국어, English, 中文, 日本語 from top to bottom: {rows:?}");
  let before=picked;let after=picker_click(&ctx,&mut picked,rows[Lang::ALL.iter().position(|l|*l==target).unwrap()]);
  assert_eq!(picked,target,"a click on {} picks it (it was {before:?})",target.native_name());
  assert!(spot(&after,|n|n.label()==Some(target.native_name())).is_none(),"the list shuts after the pick");
 }
}
