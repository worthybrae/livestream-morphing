/// RGB24 pixel buffer.
#[derive(Clone)]
pub struct RawFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RawFrame {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 3) as usize],
            width,
            height,
        }
    }

    pub fn filled(width: u32, height: u32, r: u8, g: u8, b: u8) -> Self {
        let mut frame = Self::new(width, height);
        for pixel in frame.data.chunks_exact_mut(3) {
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
        frame
    }
}

