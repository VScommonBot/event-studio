use super::*;
fn fit_text(p:&Painter,center:Pos2,text:&str,max:f32,size:f32,color:Color32){let mut font=FontId::proportional(size);let mut g=p.layout_no_wrap(text.into(),font.clone(),color);if g.size().x>max{font.size=(font.size*max/g.size().x).max(9.0);g=p.layout_no_wrap(text.into(),font,color);}p.galley(center-g.size()*0.5,g,color);}
fn paint_image(p:&Painter,t:&TextureHandle,rect:Rect,cover:bool){
 let sz=t.size_vec2();let aspect=sz.x/sz.y;let target=rect.width()/rect.height();let mut uv=Rect::from_min_max(pos2(0.0,0.0),pos2(1.0,1.0));let mut dest=rect;
 if cover{if aspect>target{let w=target/aspect;uv.min.x=(1.0-w)/2.0;uv.max.x=1.0-uv.min.x;}else{let h=aspect/target;uv.min.y=(1.0-h)/2.0;uv.max.y=1.0-uv.min.y;}}else{let scale=(rect.width()/sz.x).min(rect.height()/sz.y);dest=Rect::from_center_size(rect.center(),sz*scale);}
 p.image(t.id(),dest,uv,Color32::WHITE);
}
/// Paints the event stage and returns where the timer itself is (hover buttons go on it).
pub(super) fn render(p:&Painter,rect:Rect,s:&Settings,v:&timer::View,textures:&HashMap<String,TextureHandle>,qna:bool,running:bool)->Rect{
 let p=p.with_clip_rect(rect);let a=s.accent();let scale=(rect.width()/850.0).min(rect.height()/590.0).max(0.35);
 p.rect_filled(rect,12,Color32::from_rgb(18,27,35));
 if let Some(t)=textures.get(&s.background){paint_image(&p,t,rect,s.cover);p.rect_filled(rect,0,Color32::from_black_alpha((s.dim*255.0) as u8));}else{
  let mut mesh=egui::epaint::Mesh::default();let colors=[Color32::from_rgb(24,42,47),Color32::from_rgb(15,25,37),Color32::from_rgb(17,26,35),Color32::from_rgb(20,34,39)];
  for(pos,col)in[rect.left_top(),rect.right_top(),rect.right_bottom(),rect.left_bottom()].into_iter().zip(colors){mesh.colored_vertex(pos,col);}mesh.add_triangle(0,1,2);mesh.add_triangle(0,2,3);p.add(Shape::mesh(mesh));
  for i in 0..5{let rr=rect.width()*0.45+i as f32*46.0*scale;p.circle_stroke(pos2(rect.right()+rect.width()*0.1,rect.top()-rect.height()*0.15),rr,Stroke::new(1.0_f32,Color32::from_rgba_unmultiplied(132,226,190,9)));}
 }
 p.rect_stroke(rect,12,Stroke::new(1.0_f32,LINE),StrokeKind::Inside);
 let pad=32.0*scale;let top=rect.top()+pad;
 p.text(pos2(rect.left()+pad,top+10.0*scale),Align2::LEFT_CENTER,&s.brand,FontId::proportional(15.0*scale),a);
 let logo_box=vec2(95.0,38.0)*scale*(s.logo_size/100.0);
 let count=[&s.logo,&s.logo2].into_iter().filter(|key|textures.contains_key(*key)).count();
 let width=count as f32*logo_box.x+(count.saturating_sub(1)) as f32*12.0*scale;
 for(idx,key)in[&s.logo,&s.logo2].into_iter().filter(|key|textures.contains_key(*key)).enumerate(){if let Some(t)=textures.get(key){let left=match s.logo_position%3{0=>rect.left()+pad,1=>rect.center().x-width/2.0,_=>rect.right()-pad-width};let y=if s.logo_position<3{top+logo_box.y*0.5+25.0*scale}else{rect.bottom()-pad-logo_box.y*0.5-18.0*scale};let center=pos2(left+logo_box.x*0.5+idx as f32*(logo_box.x+12.0*scale),y);paint_image(&p,t,Rect::from_center_size(center,logo_box),false);}}
 let title_y=rect.top()+rect.height()*0.20;
 fit_text(&p,pos2(rect.center().x,title_y),&s.event,rect.width()-pad*2.0,27.0*scale,WHITE);
 fit_text(&p,pos2(rect.center().x,title_y+34.0*scale),&s.subtitle,rect.width()-pad*2.0,12.0*scale,MUTED);
 let color=phase_color(v.phase,a);
 let center=pos2(rect.center().x,rect.top()+rect.height()*0.61);
 let phase_text=match v.phase{Phase::Ready=>t("시작할 준비가 되었습니다"),Phase::Warning=>t("마무리를 준비해주세요"),Phase::End=>t("시간이 종료되었습니다"),Phase::Over=>t("설정 시간을 초과했습니다"),_=>if running{t("진행 중")}else{t("일시정지")}};
 let timer_label=if qna{"Q&A"}else if s.count_up{"ELAPSED TIME"}else{"TIME REMAINING"};
 let timer_rect=match s.mode{
  Mode::Circle=>{
   let radius=(rect.height()*0.235).min(rect.width()*0.26);let width=10.0*scale;
   ring(&p,center,radius,width,Color32::from_rgb(41,58,64));
   let steps=180;let angle=std::f32::consts::TAU*v.fill;
   if v.fill>=0.999{ring(&p,center,radius,width,color);}else if angle>0.0{let pts=(0..=steps).map(|i|{let t=-std::f32::consts::FRAC_PI_2+angle*i as f32/steps as f32;center+vec2(t.cos(),t.sin())*radius}).collect();p.add(Shape::line(pts,Stroke::new(width,color)));}
   for i in 0..60{let t=std::f32::consts::TAU*i as f32/60.0-std::f32::consts::FRAC_PI_2;let dir=vec2(t.cos(),t.sin());let len=if i%5==0{6.0}else{3.0}*scale;p.line_segment([center+dir*(radius-17.0*scale),center+dir*(radius-(17.0+len)*scale)],Stroke::new(1.0_f32,Color32::from_rgb(60,78,83)));}
   fit_text(&p,center-vec2(0.0,39.0*scale),timer_label,radius*1.65,10.0*scale,MUTED);
   fit_text(&p,center+vec2(0.0,3.0*scale),&v.text,radius*1.66,68.0*scale,WHITE);
   fit_text(&p,center+vec2(0.0,51.0*scale),if qna{t("질의응답")}else{t("발표 시간")},radius*1.6,12.0*scale,color);
   Rect::from_center_size(center,Vec2::splat(radius*2.0+width))
  },
  Mode::Bar=>{
   fit_text(&p,center-vec2(0.0,77.0*scale),timer_label,rect.width()*0.8,11.0*scale,MUTED);
   fit_text(&p,center-vec2(0.0,20.0*scale),&v.text,rect.width()*0.78,99.0*scale,WHITE);
   let bar=Rect::from_center_size(center+vec2(0.0,64.0*scale),vec2(rect.width()*0.76,21.0*scale));p.rect_filled(bar,9,Color32::from_rgb(41,58,64));
   if v.fill>0.0{let fill=Rect::from_min_size(bar.min,vec2(bar.width()*v.fill,bar.height()));p.rect_filled(fill,9,color);}
   p.text(bar.left_bottom()+vec2(0.0,18.0*scale),Align2::LEFT_TOP,if s.count_up{tf!("경과 {0}%",format!("{:.0}",v.fill*100.0))}else{tf!("남은 시간 {0}%",format!("{:.0}",v.remaining*100.0))},FontId::proportional(11.0*scale),MUTED);
   p.text(bar.right_bottom()+vec2(0.0,18.0*scale),Align2::RIGHT_TOP,if qna{"Q&A"}else{"PRESENTATION"},FontId::proportional(11.0*scale),MUTED);
   Rect::from_min_max(pos2(bar.left(),center.y-85.0*scale),pos2(bar.right(),bar.bottom()+32.0*scale))
  },
  Mode::Number=>{
   fit_text(&p,center-vec2(0.0,86.0*scale),timer_label,rect.width()*0.8,11.0*scale,MUTED);
   fit_text(&p,center,&v.text,rect.width()*0.83,134.0*scale,color);
   fit_text(&p,center+vec2(0.0,88.0*scale),if qna{t("질의응답")}else{t("발표 시간")},rect.width()*0.7,14.0*scale,MUTED);
   Rect::from_center_size(center+vec2(0.0,6.0*scale),vec2(rect.width()*0.6,200.0*scale))
  }
 };
 fit_text(&p,pos2(rect.center().x,rect.bottom()-28.0*scale),phase_text,rect.width()*0.8,12.0*scale,color);
 timer_rect.intersect(rect)
}
