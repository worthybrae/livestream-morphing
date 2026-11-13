//! Brush-local paint texture. Nothing here depends on frame time or screen
//! position: cached relief and broken pigment travel with the stroke.
use crate::Stamp;

#[derive(Clone,Copy,PartialEq)]
pub enum Material { Plain, Loaded, Scumble, Swept, Mixed }
impl Material {
    pub fn parse(value:&str)->Option<Self> {
        match value {"plain"=>Some(Self::Plain),"loaded"=>Some(Self::Loaded),
            "scumble"=>Some(Self::Scumble),"swept"=>Some(Self::Swept),"mixed"=>Some(Self::Mixed),_=>None}
    }
}

fn noise(x:f32,y:f32,seed:u32)->f32 {
    let hash=|ix:i32,iy:i32| {
        let mut n=(ix as u32).wrapping_mul(374761393)^(iy as u32).wrapping_mul(668265263)^seed;
        n=(n^(n>>13)).wrapping_mul(1274126177);
        (n^(n>>16)) as f32/u32::MAX as f32
    };
    let (ix,iy)=(x.floor() as i32,y.floor() as i32);
    let (fx,fy)=(x-x.floor(),y-y.floor());
    let (a,b)=(fx*fx*(3.-2.*fx),fy*fy*(3.-2.*fy));
    let top=hash(ix,iy)*(1.-a)+hash(ix+1,iy)*a;
    let bottom=hash(ix,iy+1)*(1.-a)+hash(ix+1,iy+1)*a;
    top*(1.-b)+bottom*b
}

/// A line drawn with the point of the brush: it swells and thins with
/// pressure, wanders a little, and tapers out at both ends.
fn line(p:&[f32])->Stamp {
    let (length,width,angle,seed)=(p[3],p[4],p[5],p[6]);
    let (sn,cs)=angle.sin_cos();
    let reach=width*1.6+length*0.05+1.;
    let bx=(cs.abs()*length+sn.abs()*reach).ceil() as i32+2;
    let by=(sn.abs()*length+cs.abs()*reach).ceil() as i32+2;
    let (w,h)=((bx*2+1) as usize,(by*2+1) as usize);
    let mut pixels=vec![[0,0];w*h];
    for y in -by..=by {for x in -bx..=bx {
        let u=(x as f32*cs+y as f32*sn)/length;
        if u.abs()>=1. {continue;}
        let wander=(1.-u*u)*(seed-0.5)*0.07*length+(u*(4.+seed*5.)+seed*40.).sin()*0.3;
        let across=(-x as f32*sn+y as f32*cs-wander)/width;
        let taper=(1.-u.abs().powf(2.2)).max(0.).powf(0.6);
        let pressure=taper*(0.72+0.28*(u*2.1+seed*6.28).sin());
        let cover=((pressure-across.abs())*width+0.35).clamp(0.,1.);
        if cover==0. {continue;}
        let alpha=(cover*0.82*255.).round() as i16;
        pixels[(y+by) as usize*w+(x+bx) as usize]=[alpha,alpha*3];
    }}
    Stamp::new(w,h,-bx,-by,pixels)
}

/// Bow and S-curve of a long stroke ride in the high bits of its seed; the
/// planner (render.py `bends`) packs them the same way. Returns the plain seed too.
fn bends(p:&[f32])->(f32,f32,f32) {
    let packed=p[6]*16384.;let code=packed.floor() as i32;
    (((code>>7)-64) as f32/63.*0.4,((code&127)-64) as f32/63.*0.4,packed.fract())
}

/// Half extents of the rectangle a bent stroke needs.
fn long_box(length:f32,reach:f32,sn:f32,cs:f32)->(i32,i32) {
    ((cs.abs()*length+sn.abs()*reach).ceil() as i32+3,(sn.abs()*length+cs.abs()*reach).ceil() as i32+3)
}

/// One long loaded pull: it lands full and wide, follows its bend, and the
/// bristles open into dry streaks as the paint runs out.
fn pull(p:&[f32])->Stamp {
    let (length,width,angle)=(p[3],p[4],p[5]);
    let (bow,snake,seed)=bends(p);
    let (sn,cs)=angle.sin_cos();
    let (bx,by)=long_box(length,width*1.5+(bow.abs()+snake.abs()*0.39)*length,sn,cs);
    let (w,h)=((bx*2+1) as usize,(by*2+1) as usize);
    let mut pixels=vec![[0,0];w*h];
    let side_light=sn*0.60+cs*0.80;
    let grain=(seed*100000.) as u32;
    for y in -by..=by {for x in -bx..=bx {
        let u=(x as f32*cs+y as f32*sn)/length;
        if u.abs()>=1. {continue;}
        let across=(-x as f32*sn+y as f32*cs-(1.-u*u)*(bow+snake*u)*length)/width;
        if across.abs()>1.25 {continue;}
        let run=(u+1.)*0.5;
        let head=(run/0.05).clamp(0.,1.).sqrt();
        let tail=((1.-run)/(0.16+seed*0.14)).clamp(0.,1.).sqrt();
        let pressure=(0.96-run*0.30+(u*(5.+seed*4.)+seed*22.).sin()*0.05)*head*tail;
        let edge=(pressure-across.abs())*width;
        if edge < -0.25 {continue;}
        let clump=noise(run*length*0.045,across*width*0.40,grain);
        let phase=across*width*(0.95+seed*0.8)+seed*27.+clump*2.6;
        let ridge=phase.sin();
        let cover=(edge+ridge*0.22).clamp(0.,1.);
        if cover==0. {continue;}
        // Past the middle the brush is emptying: only the fullest bristles still mark.
        let empty=((run-0.40)/0.60).clamp(0.,1.);
        let gap=noise(run*length*0.028,across*width*0.95,grain^0x9e37);
        let held=(1.-(empty*0.92-(gap*0.6+(ridge*0.5+0.5)*0.4))*5.).clamp(0.,1.);
        let edge_ridge=(-((across.abs()/pressure.max(0.05)-0.84)*9.).powi(2)).exp();
        let grooves=(clump*1.4-0.25).clamp(0.,1.)*(0.35+seed*0.65);
        let relief=side_light*(phase.cos()*8.*grooves+across.signum()*edge_ridge*15.)
            +(clump-0.5)*12.+(1.-u*u)*2.-empty*6.+(seed-0.5)*4.;
        let alpha=(cover*0.97*held*255.).round().clamp(0.,255.) as i16;
        let shade=relief.round().clamp(-40.,40.) as i16;
        pixels[(y+by) as usize*w+(x+bx) as usize]=[alpha,alpha*shade];
    }}
    Stamp::new(w,h,-bx,-by,pixels)
}

/// A long line drawn with the point of the brush along a contour: pressed in
/// at the start, thinning as it travels, breaking up where the paint gives out.
/// The fraction of its coat is a phase: the swell of its weight travels with it.
fn contour(p:&[f32])->Stamp {
    let (length,width,angle)=(p[3],p[4],p[5]);
    let (bow,snake,seed)=bends(p);
    let (sn,cs)=angle.sin_cos();
    let (bx,by)=long_box(length,width*1.3+1.+(bow.abs()+snake.abs()*0.39)*length,sn,cs);
    let (w,h)=((bx*2+1) as usize,(by*2+1) as usize);
    let mut pixels=vec![[0,0];w*h];
    let grain=(seed*100000.) as u32;
    // A contour thins as it travels; a restated marking or pole keeps its width.
    // The upper half of the coat's fraction marks a border: even, unbroken, still.
    let border=p[11].fract()>=0.5;
    let (thinning,swell)=if border {(0.,0.10)} else if p[11]>=10. {(0.16,0.07)} else {(0.42,0.26)};
    let flow=if border {0.} else {p[11].fract()*2.};
    let at=|u:f32| {let bend=(1.-u*u)*(bow+snake*u)*length;(u*length*cs-bend*sn,u*length*sn+bend*cs)};
    // The rectangle around a slanted line is nearly all empty, and a moving line
    // is redrawn every update: visit only the pixels beside each short piece of it.
    let pieces=((length/24.).ceil() as i32).max(1);
    let pad=width*1.3+3.;
    for piece in 0..pieces {
        let (u0,u1)=(-1.+2.*piece as f32/pieces as f32,-1.+2.*(piece+1) as f32/pieces as f32);
        let (a,b,m)=(at(u0),at(u1),at((u0+u1)*0.5));
        let x0=((a.0.min(b.0).min(m.0)-pad).floor() as i32).max(-bx);
        let x1=((a.0.max(b.0).max(m.0)+pad).ceil() as i32).min(bx);
        let y0=((a.1.min(b.1).min(m.1)-pad).floor() as i32).max(-by);
        let y1=((a.1.max(b.1).max(m.1)+pad).ceil() as i32).min(by);
        for y in y0..=y1 {for x in x0..=x1 {
            let u=(x as f32*cs+y as f32*sn)/length;
            if u.abs()>=1. {continue;}
            let across=(-x as f32*sn+y as f32*cs-(1.-u*u)*(bow+snake*u)*length)/width;
            if across.abs()>1.+0.5/width {continue;}
            let run=(u+1.)*0.5;
            let taper=if border {((1.-u.abs())/0.04).clamp(0.,1.).sqrt()}
                else {(run/0.04).clamp(0.,1.).powf(0.7)*((1.-run)/0.35).clamp(0.,1.).powf(0.8)};
            let pressure=taper*(0.97-run*thinning+(u*4.4+(seed-flow)*6.28).sin()*swell).clamp(0.15,1.);
            let cover=((pressure-across.abs())*width+0.4*taper.sqrt()).clamp(0.,1.);
            if cover==0. {continue;}
            let empty=if border {0.} else {((run-0.5)/0.5).clamp(0.,1.)};
            let gap=noise(run*length*0.05,across*width*1.2,grain);
            let held=(1.-(empty*0.85-gap)*4.).clamp(0.,1.);
            let alpha=(cover*0.94*held*255.).round() as i16;
            pixels[(y+by) as usize*w+(x+bx) as usize]=[alpha,alpha*3];
        }}
    }
    Stamp::new(w,h,-bx,-by,pixels)
}

pub fn stamp(p:&[f32],material:Material)->Stamp {
    // Coat codes above the approved coats; 4, 5 and 6 belong to the finishing pass.
    if p[11]==8. {return line(p);}
    if p[11]>=9. && p[11]<11. {return contour(p);}
    if p[11]==1.5 {return pull(p);}
    if p[11]<0. || p[4]<2. {return crate::make_stamp(p);}
    let (mut length,mut width,angle,seed)=(p[3],p[4],p[5],p[6]);
    // One hand, several ways of putting paint down: broad passages are scrubbed
    // thin or swept, middle marks are mostly loaded, small ones always loaded.
    let material=if material==Material::Mixed {
        let pick=(seed*53.17).fract();
        if width>12. {if pick<0.5 {Material::Scumble} else {Material::Swept}}
        else if width>4. {if pick<0.45 {Material::Loaded} else if pick<0.75 {Material::Swept} else {Material::Scumble}}
        else {Material::Loaded}
    } else {material};
    // Keep tiny accents compact and opaque. Large passages can carry a more
    // assertive gesture without changing the planner's source-color sample.
    let detail=((width-1.)/4.).clamp(0.,1.);
    match material {
        Material::Loaded=>{length*=1.+detail*(seed-0.5)*0.22;width*=1.+detail*0.06;},
        Material::Scumble=>{length*=1.+detail*0.12;width*=1.+detail*0.05;},
        Material::Swept=>{length*=1.+detail*0.32;width*=1.-detail*0.07;},
        Material::Plain|Material::Mixed=>return crate::make_stamp(p),
    }
    let (sn,cs)=angle.sin_cos();
    let bx=(cs.abs()*length+sn.abs()*width*1.6).ceil() as i32+3;
    let by=(sn.abs()*length+cs.abs()*width*1.6).ceil() as i32+3;
    let (w,h)=((bx*2+1) as usize,(by*2+1) as usize);
    let mut pixels=vec![[0,0];w*h];
    // Fixed light direction makes neighboring deposits read as one material.
    let side_light=sn*0.60+cs*0.80;
    for y in -by..=by {for x in -bx..=bx {
        let u=(x as f32*cs+y as f32*sn)/length;
        let v=(-x as f32*sn+y as f32*cs)/width;
        if u.abs()>=1. {continue;}
        let belly=1.-u*u;
        let curve=match material {
            Material::Swept=>belly*((seed-0.5)*0.70+u*0.30),
            _=>belly*((seed-0.5)*0.48+u*0.10),
        };
        let across=v-curve*detail;
        // Unequal ends, a loaded middle, and longitudinal pressure variation.
        let head=((u+1.)/0.24).clamp(0.,1.).sqrt();
        let tail=((1.-u)/(0.36+seed*0.22)).clamp(0.,1.).sqrt();
        let wobble=(u*(7.+seed*4.)+seed*22.).sin()*0.045
            +(u*17.+seed*63.).sin()*0.020;
        let pressure=(0.84-u*0.12+belly*0.10+wobble*detail)*head*tail;
        let edge=(pressure-across.abs())*width;
        // Even the highest bristle ridge cannot reach this pixel. Avoid noise
        // and relief calculations in the empty corners of the brush rectangle.
        if edge < -0.20*detail-0.001 {continue;}
        let clump=noise(u*length*0.13,across*width*0.39,(seed*100000.) as u32);
        let phase=across*width*(1.05+seed*0.9)+seed*27.+u*(1.+seed*2.)+clump*2.5;
        let ridge=phase.sin();
        let bundle=(across*9.+u*0.9+seed*31.).sin();
        let cover=(edge+ridge*0.18*detail).clamp(0.,1.);
        if cover==0. {continue;}
        let tip=((u-0.10)/0.9).clamp(0.,1.);
        let edge_ridge=(-((across.abs()/pressure.max(0.05)-0.84)*9.).powi(2)).exp();
        let (deposit,light)=match material {
            Material::Loaded=>{
                let dry=tip*(1.-ridge.max(0.))*0.25*detail;
                let grooves=(clump*1.4-0.25).clamp(0.,1.)*(0.35+seed*0.65);
                let relief=side_light*(phase.cos()*8.*grooves+across.signum()*edge_ridge*15.)
                    +(clump-0.5)*12.+bundle*1.5+belly*2.-tip*5.;
                (0.98-dry,relief*detail+(seed-0.5)*4.)
            },
            Material::Scumble=>{
                let grain=noise(u*length*0.65,across*width*1.1,(seed*730001.) as u32);
                // Broken islands of pigment reveal earlier painted coats.
                // The underpainting supplies continuity instead of raw video.
                let breakup=((clump*0.65+grain*0.35-0.32)*2.8).clamp(0.,1.);
                let dry=(0.35+breakup*0.64)*(1.-tip*0.30*(1.-ridge.max(0.)));
                (1.-detail*(1.-dry),((grain-0.5)*12.+(clump-0.5)*8.-tip*3.)*detail)
            },
            Material::Swept=>{
                let drag=tip*(1.-ridge.max(0.))*0.30*detail;
                let load=(across*4.+seed*15.+u*0.6).sin()*6.+(clump-0.5)*8.;
                (0.97-drag,(side_light*phase.cos()*8.*clump+load+edge_ridge*8.-tip*4.)*detail)
            },
            Material::Plain|Material::Mixed=>unreachable!(),
        };
        let alpha=(cover*deposit*255.).round().clamp(0.,255.) as i16;
        let shade=light.round().clamp(-40.,40.) as i16;
        pixels[(y+by) as usize*w+(x+bx) as usize]=[alpha,alpha*shade];
    }}
    Stamp::new(w,h,-bx,-by,pixels)
}
