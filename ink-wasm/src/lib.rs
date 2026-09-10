//! Browser ABI around the very same effect source used by the native stream.
use std::cell::RefCell;
#[macro_export]
macro_rules! register_effect { ($ty:ty) => {}; }
pub mod registry {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../livestream-morphing-rs/src/effect_types.rs"));
    pub fn default_params(defs: &[ParamDef]) -> ParamValues {
        defs.iter().map(|p| (p.id.to_string(), p.default)).collect()
    }
}
pub mod effects {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../livestream-morphing-rs/src/effects/raw_frame.rs"));
    pub mod distortion { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../livestream-morphing-rs/src/effects/distortion.rs")); }
    pub mod living_ink { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../livestream-morphing-rs/src/effects/living_ink.rs")); }
}
use effects::{RawFrame, living_ink::LivingInk};
use registry::{Effect, FrameCtx, ParamValues};
struct Renderer {
    ink: LivingInk,
    frame: RawFrame,
    rgba: Vec<u8>,
    params: ParamValues,
    definitions: Vec<u8>,
}
thread_local! { static RENDERER: RefCell<Option<Renderer>> = const {RefCell::new(None)}; }

#[no_mangle]
pub extern "C" fn ink_init(width: u32, height: u32) -> u32 {
    if width < 3 || height < 3 || width > 1920 || height > 1080 { return 0; }
    let mut ink=LivingInk::default(); ink.init(width,height);
    let defs=ink.params();
    let params=serde_json::from_str(include_str!("../../livestream-morphing-rs/src/living_ink_preset.json")).unwrap();
    RENDERER.with(|s| *s.borrow_mut()=Some(Renderer {
        ink,frame:RawFrame::new(width,height),rgba:vec![0;width as usize*height as usize*4],
        params,definitions:serde_json::to_vec(&defs).unwrap(),
    }));
    1
}
#[no_mangle]
pub extern "C" fn ink_pixels() -> *mut u8 {
    RENDERER.with(|s|s.borrow_mut().as_mut().map_or(std::ptr::null_mut(),|r|r.rgba.as_mut_ptr()))
}
#[no_mangle]
pub extern "C" fn ink_definitions() -> *const u8 {
    RENDERER.with(|s|s.borrow().as_ref().map_or(std::ptr::null(),|r|r.definitions.as_ptr()))
}
#[no_mangle]
pub extern "C" fn ink_definitions_len() -> usize {
    RENDERER.with(|s|s.borrow().as_ref().map_or(0,|r|r.definitions.len()))
}
#[no_mangle]
pub extern "C" fn ink_set_param(index: usize, value: f32) -> u32 {
    RENDERER.with(|s| {
        let mut state=s.borrow_mut(); let Some(r)=state.as_mut() else {return 0};
        let defs=r.ink.params(); let Some(def)=defs.get(index) else {return 0};
        if !value.is_finite() || value<def.min || value>def.max {return 0;}
        r.params.insert(def.id.to_string(),value); 1
    })
}
#[no_mangle]
pub extern "C" fn ink_clear_history() {
    RENDERER.with(|s|if let Some(r)=s.borrow_mut().as_mut(){r.ink.clear_history();});
}
#[no_mangle]
pub extern "C" fn ink_render(seconds: f64, delta: f32) -> u32 {
    if !seconds.is_finite() || !delta.is_finite() || delta<0.0 {return 0;}
    RENDERER.with(|s| {
        let mut state=s.borrow_mut(); let Some(r)=state.as_mut() else {return 0};
        for (rgb,rgba) in r.frame.data.chunks_exact_mut(3).zip(r.rgba.chunks_exact(4)) {rgb.copy_from_slice(&rgba[..3]);}
        let ctx=FrameCtx{width:r.frame.width,height:r.frame.height,frame_number:0,delta_seconds:delta};
        r.ink.apply_at_time(&mut r.frame,&r.params,&ctx,seconds);
        for (rgb,rgba) in r.frame.data.chunks_exact(3).zip(r.rgba.chunks_exact_mut(4)) {rgba[..3].copy_from_slice(rgb);rgba[3]=255;}
        1
    })
}
#[cfg(test)]
mod abi_tests {
    use super::*;
    #[test]
    fn rejects_invalid_inputs_and_renders_opaque_pixels() {
        assert_eq!(ink_init(0,1080),0);assert_eq!(ink_init(4096,2160),0);
        assert_eq!(ink_init(64,32),1);
        assert_eq!(ink_set_param(999,1.0),0);assert_eq!(ink_set_param(0,f32::NAN),0);
        assert_eq!(ink_render(f64::NAN,0.033),0);
        RENDERER.with(|s| {let mut state=s.borrow_mut();let r=state.as_mut().unwrap();for (i,p) in r.rgba.chunks_exact_mut(4).enumerate(){p.fill(if i%64<32{0}else{255});}});
        assert_eq!(ink_render(1.0,1.0/30.0),1);
        RENDERER.with(|s|{let state=s.borrow();let r=state.as_ref().unwrap();assert!(r.rgba.chunks_exact(4).all(|p|p[3]==255));assert!(r.rgba.chunks_exact(4).any(|p|p[0]>100));});
    }
}
