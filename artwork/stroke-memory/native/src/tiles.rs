//! Persistent exact canvas with conservative 64x64 dirty-tile replay.
use super::{blend_scalar, material, HashMap, HashSet, Stamp};
use std::time::Instant;

pub const TILE: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect { pub x0:usize,pub y0:usize,pub x1:usize,pub y1:usize }
impl Rect {
    fn overlaps(self,other:Self)->bool {
        self.x0<other.x1 && other.x0<self.x1 && self.y0<other.y1 && other.y0<self.y1
    }
}

pub struct MarkState { pub values:[f32;12],pub bounds:Rect }
pub struct TileResult<'a> {
    pub rgb:&'a [u8],pub mutated_pixels:u64,pub dirty_micros:u64,
    pub full_raster_micros:u64,pub full_fallback:bool,
}
pub struct TileGrid {
    width:usize,height:usize,columns:usize,
    previous:HashMap<u32,MarkState>,order:Vec<u32>,
    members:Vec<Vec<u32>>,dirty:Vec<bool>,initialized:bool,force_full:bool,
}
pub struct TilePainter {
    grid:TileGrid,canvas:Vec<u8>,rgb:Vec<u8>,cache:HashMap<u32,Stamp>,
    material:material::Material,
}

impl TileGrid {
    fn new(width:usize,height:usize)->Self {
        let columns=width.div_ceil(TILE);let rows=height.div_ceil(TILE);
        Self {width,height,columns,previous:HashMap::default(),order:Vec::new(),
            members:(0..columns*rows).map(|_|Vec::new()).collect(),dirty:vec![false;columns*rows],
            initialized:false,force_full:false}
    }
    fn tile_rect(&self,index:usize)->Rect {
        let x0=index%self.columns*TILE;let y0=index/self.columns*TILE;
        Rect {x0,y0,x1:(x0+TILE).min(self.width),y1:(y0+TILE).min(self.height)}
    }
    fn mark(&mut self,rect:Rect) {
        if rect.x0>=rect.x1 || rect.y0>=rect.y1 {return;}
        let x0=rect.x0/TILE;let x1=(rect.x1-1)/TILE;
        let y0=rect.y0/TILE;let y1=(rect.y1-1)/TILE;
        for y in y0..=y1 {for x in x0..=x1 {self.dirty[y*self.columns+x]=true;}}
    }
    fn all(&mut self) {self.dirty.fill(true);}
    fn clear(&mut self) {
        self.previous.clear();self.order.clear();self.members.iter_mut().for_each(Vec::clear);
        self.dirty.fill(false);self.initialized=false;self.force_full=false;
    }
}

fn geometry_changed(a:&[f32;12],b:&[f32;12])->bool {
    [3,4,5,6,11].into_iter().any(|index|a[index].to_bits()!=b[index].to_bits())
}
fn packet_changed(a:&[f32;12],b:&[f32;12])->bool {
    a.iter().zip(b).any(|(x,y)|x.to_bits()!=y.to_bits())
}
fn bounds(width:usize,height:usize,p:&[f32],stamp:&Stamp)->Rect {
    let x=p[1].floor() as i32+stamp.ox;let y=p[2].floor() as i32+stamp.oy;
    Rect {x0:(x+1).max(0).min(width as i32) as usize,
        y0:(y+1).max(0).min(height as i32) as usize,
        x1:(x+stamp.w as i32).max(0).min(width as i32) as usize,
        y1:(y+stamp.h as i32).max(0).min(height as i32) as usize}
}
fn draw_region(canvas:&mut [u8],canvas_width:usize,rect:Rect,p:&[f32],stamp:&Stamp) {
    let cx=p[1].floor() as i32+stamp.ox;let cy=p[2].floor() as i32+stamp.oy;
    let y0=(rect.y0 as i32-cy).max(1) as usize;
    let y1=(rect.y1 as i32-cy).clamp(0,stamp.h as i32) as usize;
    let x0=(rect.x0 as i32-cx).max(1) as usize;
    let x1=(rect.x1 as i32-cx).clamp(0,stamp.w as i32) as usize;
    if x0>=x1 || y0>=y1 {return;}
    let color=[p[7].clamp(0.,255.) as i32,p[8].clamp(0.,255.) as i32,
        p[9].clamp(0.,255.) as i32,255];
    for sy in y0..y1 {
        let start=x0.max(stamp.spans[sy].0);let end=x1.min(stamp.spans[sy].1);
        if start>=end {continue;}
        let at=((cy+sy as i32) as usize*canvas_width+(cx+start as i32) as usize)*4;
        blend_scalar(&mut canvas[at..at+(end-start)*4],
            &stamp.prepared[sy*stamp.w+start..sy*stamp.w+end],color);
    }
}

impl TilePainter {
    pub fn new(width:usize,height:usize,style:&str)->Option<Self> {
        if width==0 || height==0 || width>8192 || height>8192 || width*height>16_777_216 {return None;}
        Some(Self {grid:TileGrid::new(width,height),canvas:vec![0;width*height*4],
            rgb:vec![0;width*height*3],cache:HashMap::default(),material:material::Material::parse(style)?})
    }
    pub fn clear(&mut self) {self.grid.clear();self.cache.clear();}
    pub fn force_full_invalidation(&mut self) {self.grid.force_full=true;}
    pub fn paint(&mut self,values:&[f32])->Option<TileResult<'_>> {
        if values.len()%12!=0 || values.len()>100_000*12 {return None;}
        if values.chunks_exact(12).any(|p|!p.iter().all(|v|v.is_finite()) ||
            p[3]<=0. || p[4]<=0. || p[3]>512. || p[4]>512.) {return None;}
        let started=Instant::now();self.grid.dirty.fill(false);
        let mut ids=HashSet::with_capacity_and_hasher(values.len()/12,Default::default());
        let mut order=Vec::with_capacity(values.len()/12);
        for p in values.chunks_exact(12) {
            let id=p[0] as u32;if !ids.insert(id) {return None;}order.push(id);
            let mut value=[0.;12];value.copy_from_slice(p);
            let rebuild=self.grid.previous.get(&id).map(|old|geometry_changed(&old.values,&value)).unwrap_or(true);
            if rebuild {
                let mut stamp=material::stamp(p,self.material);stamp.prepare(p);self.cache.insert(id,stamp);
            } else {let stamp=self.cache.get_mut(&id)?;stamp.prepare(p);}
            let next=bounds(self.grid.width,self.grid.height,p,self.cache.get(&id)?);
            if let Some(old)=self.grid.previous.get(&id) {
                if packet_changed(&old.values,&value) {let old_bounds=old.bounds;self.grid.mark(old_bounds);self.grid.mark(next);}
            } else {self.grid.mark(next);}
        }
        let removed:Vec<_>=self.grid.previous.iter().filter(|(id,_)|!ids.contains(id))
            .map(|(id,state)|(*id,state.bounds)).collect();
        for (id,old) in removed {self.grid.mark(old);self.cache.remove(&id);}
        let retained_old:Vec<_>=self.grid.order.iter().filter(|id|ids.contains(id)).copied().collect();
        let retained_new:Vec<_>=order.iter().filter(|id|self.grid.previous.contains_key(id)).copied().collect();
        let mut full_fallback=!self.grid.initialized || self.grid.force_full;
        if retained_old!=retained_new {
            let positions:HashMap<_,_>=retained_new.iter().enumerate().map(|(at,id)|(*id,at)).collect();
            'outer: for (left,a) in retained_old.iter().enumerate() {for b in retained_old.iter().skip(left+1) {
                if positions[a]>positions[b] && self.grid.previous[a].bounds.overlaps(self.grid.previous[b].bounds) {
                    full_fallback=true;break 'outer;
                }
            }}
        }
        if full_fallback {self.grid.all();}
        self.grid.members.iter_mut().for_each(Vec::clear);
        let mut current=HashMap::default();
        for (id,p) in order.iter().copied().zip(values.chunks_exact(12)) {
            let mut value=[0.;12];value.copy_from_slice(p);
            let area=bounds(self.grid.width,self.grid.height,p,self.cache.get(&id)?);
            if area.x0<area.x1 && area.y0<area.y1 {
                for y in area.y0/TILE..=(area.y1-1)/TILE {for x in area.x0/TILE..=(area.x1-1)/TILE {
                    self.grid.members[y*self.grid.columns+x].push(id);
                }}
            }
            current.insert(id,MarkState {values:value,bounds:area});
        }
        self.grid.previous=current;self.grid.order=order;self.grid.initialized=true;self.grid.force_full=false;
        let dirty_indices:Vec<_>=self.grid.dirty.iter().enumerate().filter_map(|(i,v)|v.then_some(i)).collect();
        let mut mutated=0u64;
        for index in dirty_indices {
            let rect=self.grid.tile_rect(index);
            for y in rect.y0..rect.y1 {for x in rect.x0..rect.x1 {
                self.canvas[(y*self.grid.width+x)*4..(y*self.grid.width+x+1)*4].copy_from_slice(&[81,83,74,255]);
            }}
            for &id in &self.grid.members[index] {
                let state=&self.grid.previous[&id];draw_region(&mut self.canvas,self.grid.width,rect,&state.values,&self.cache[&id]);
            }
            for y in rect.y0..rect.y1 {for x in rect.x0..rect.x1 {
                let src=(y*self.grid.width+x)*4;let dst=(y*self.grid.width+x)*3;
                if self.rgb[dst..dst+3]!=self.canvas[src..src+3] {mutated+=1;}
                self.rgb[dst..dst+3].copy_from_slice(&self.canvas[src..src+3]);
            }}
        }
        let elapsed=started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        Some(TileResult {rgb:&self.rgb,mutated_pixels:mutated,
            dirty_micros:if full_fallback {0}else{elapsed},
            full_raster_micros:if full_fallback {elapsed}else{0},full_fallback})
    }
    #[cfg(test)] fn member_slots(&self)->usize {self.grid.members.iter().map(Vec::len).sum()}
}

#[cfg(test)] mod tests {
    use super::*;use crate::Painter;
    fn mark(id:f32,x:f32,y:f32)->[f32;12] {[id,x,y,18.,7.,0.31,id*0.17,160.,80.,190.,0.83,1.]}
    fn verify_sequence(width:usize,height:usize,sequence:Vec<Vec<[f32;12]>>) {
        let mut tiles=TilePainter::new(width,height,"loaded").unwrap();
        for (at,marks) in sequence.into_iter().enumerate() {
            let values=marks.concat();let actual=tiles.paint(&values).unwrap().rgb.to_vec();
            let mut full=Painter::new(width,height,"loaded").unwrap();let expected=full.paint(&values,false).unwrap();
            assert_eq!(actual,expected,"transaction {at}");
        }
    }
    #[test] fn incremental_tiles_match_static_motion_recolor_add_remove_clip_and_order() {
        let p=mark(1.,25.25,22.75);let mut moved=p;moved[1]+=1.25;moved[2]-=0.75;
        let mut recolored=moved;recolored[7]=12.;recolored[8]=220.;
        let q=mark(2.,31.6,24.2);let mut clipped=q;clipped[1]=-3.2;clipped[2]=60.4;
        verify_sequence(79,61,vec![vec![],vec![p],vec![p],vec![moved],vec![recolored],
            vec![recolored,q],vec![q],vec![clipped],vec![p,q],vec![q,p]]);
    }
    #[test] fn odd_dimensions_and_forced_full_invalidation_are_exact() {
        let p=mark(1.,64.4,63.8);let values=vec![p].concat();let mut tiles=TilePainter::new(131,67,"loaded").unwrap();
        let first=tiles.paint(&values).unwrap();assert!(first.full_fallback);
        let static_frame=tiles.paint(&values).unwrap();assert_eq!(static_frame.mutated_pixels,0);
        tiles.force_full_invalidation();let forced=tiles.paint(&values).unwrap();
        assert!(forced.full_fallback);assert_eq!(forced.mutated_pixels,0);
        let mut full=Painter::new(131,67,"loaded").unwrap();assert_eq!(forced.rgb,full.paint(&values,false).unwrap());
    }
    #[test] fn member_lists_stay_bounded_across_many_transactions() {
        let mut tiles=TilePainter::new(129,97,"loaded").unwrap();
        for frame in 0..1000 {
            let marks:Vec<_>=(0..24).map(|id|mark(id as f32+1.,(id*5) as f32+(frame%3) as f32*0.2,(id*7%97) as f32)).collect();
            tiles.paint(&marks.concat()).unwrap();assert!(tiles.member_slots()<=24*4);
        }
    }
}
