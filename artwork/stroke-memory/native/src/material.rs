//! Brush-local paint texture. Nothing here depends on frame time or screen
//! position: cached relief and broken pigment travel with the stroke.
use crate::Stamp;

#[derive(Clone,Copy,PartialEq)]
pub enum Material { Plain, Loaded, Scumble, Swept }
impl Material {
    pub fn parse(value:&str)->Option<Self> {
        match value {"plain"=>Some(Self::Plain),"loaded"=>Some(Self::Loaded),
            "scumble"=>Some(Self::Scumble),"swept"=>Some(Self::Swept),_=>None}
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

pub fn stamp(p:&[f32],material:Material)->Stamp {
    if p[11]<0. || p[4]<2. {return crate::make_stamp(p);}
    let (mut length,mut width,angle,seed)=(p[3],p[4],p[5],p[6]);
    // Keep tiny accents compact and opaque. Large passages can carry a more
    // assertive gesture without changing the planner's source-color sample.
    let detail=((width-1.)/4.).clamp(0.,1.);
    match material {
        Material::Loaded=>{length*=1.+detail*(seed-0.5)*0.22;width*=1.+detail*0.06;},
        Material::Scumble=>{length*=1.+detail*0.12;width*=1.+detail*0.05;},
        Material::Swept=>{length*=1.+detail*0.32;width*=1.-detail*0.07;},
        Material::Plain=>return crate::make_stamp(p),
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
        let clump=noise(u*length*0.13,across*width*0.39,(seed*100000.) as u32);
        let grain=noise(u*length*0.65,across*width*1.1,(seed*730001.) as u32);
        let phase=across*width*(1.05+seed*0.9)+seed*27.+u*(1.+seed*2.)+clump*2.5;
        let ridge=phase.sin();
        let bundle=(across*9.+u*0.9+seed*31.).sin();
        let edge=(pressure-across.abs())*width;
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
            Material::Plain=>unreachable!(),
        };
        let alpha=(cover*deposit*255.).round().clamp(0.,255.) as i16;
        let shade=light.round().clamp(-40.,40.) as i16;
        pixels[(y+by) as usize*w+(x+bx) as usize]=[alpha,alpha*shade];
    }}
    Stamp {w,h,ox:-bx,oy:-by,pixels,prepared:Vec::new(),key:None}
}
