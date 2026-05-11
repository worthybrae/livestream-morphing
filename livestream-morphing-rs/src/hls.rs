use std::collections::VecDeque;

pub struct Segment {
    pub id: String,
    pub data: Vec<u8>,
    pub duration: f32,
}

pub struct HlsBuffer {
    segments: VecDeque<Segment>,
    max_segments: usize,
    sequence: u64,
}

impl HlsBuffer {
    pub fn new(max_segments: usize) -> Self {
        Self {
            segments: VecDeque::new(),
            max_segments,
            sequence: 0,
        }
    }

    pub fn push_segment(&mut self, id: String, data: Vec<u8>) {
        if self.segments.len() >= self.max_segments {
            self.segments.pop_front();
            self.sequence += 1;
        }
        self.segments.push_back(Segment {
            id,
            data,
            duration: 6.0,
        });
    }

    pub fn get_segment(&self, id: &str) -> Option<&[u8]> {
        self.segments
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.data.as_slice())
    }

    pub fn generate_playlist(&self) -> String {
        let mut m3u8 = format!(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n#EXT-X-MEDIA-SEQUENCE:{}\n",
            self.sequence
        );
        for seg in &self.segments {
            m3u8.push_str(&format!(
                "#EXTINF:{:.1},\n/api/segments/{}.ts\n",
                seg.duration, seg.id
            ));
        }
        m3u8
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn media_sequence(&self) -> u64 {
        self.sequence
    }

    pub fn clear(&mut self) {
        self.sequence += self.segments.len() as u64;
        self.segments.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_retrieve_segment() {
        let mut buf = HlsBuffer::new(10);
        buf.push_segment("001".into(), vec![1, 2, 3]);
        assert_eq!(buf.get_segment("001"), Some([1u8, 2, 3].as_slice()));
        assert_eq!(buf.segment_count(), 1);
    }

    #[test]
    fn evicts_oldest_when_full() {
        let mut buf = HlsBuffer::new(3);
        buf.push_segment("1".into(), vec![1]);
        buf.push_segment("2".into(), vec![2]);
        buf.push_segment("3".into(), vec![3]);
        buf.push_segment("4".into(), vec![4]);
        assert_eq!(buf.get_segment("1"), None);
        assert_eq!(buf.get_segment("4"), Some([4u8].as_slice()));
        assert_eq!(buf.segment_count(), 3);
    }

    #[test]
    fn media_sequence_increments_on_eviction() {
        let mut buf = HlsBuffer::new(2);
        buf.push_segment("1".into(), vec![]);
        buf.push_segment("2".into(), vec![]);
        assert_eq!(buf.media_sequence(), 0);
        buf.push_segment("3".into(), vec![]);
        assert_eq!(buf.media_sequence(), 1);
        buf.push_segment("4".into(), vec![]);
        assert_eq!(buf.media_sequence(), 2);
    }

    #[test]
    fn playlist_format() {
        let mut buf = HlsBuffer::new(10);
        buf.push_segment("100".into(), vec![]);
        buf.push_segment("101".into(), vec![]);
        let playlist = buf.generate_playlist();
        assert!(playlist.contains("#EXTM3U"));
        assert!(playlist.contains("#EXT-X-MEDIA-SEQUENCE:0"));
        assert!(playlist.contains("/api/segments/100.ts"));
        assert!(playlist.contains("/api/segments/101.ts"));
    }

    #[test]
    fn empty_playlist() {
        let buf = HlsBuffer::new(10);
        let playlist = buf.generate_playlist();
        assert!(playlist.contains("#EXTM3U"));
        assert!(!playlist.contains("#EXTINF"));
    }
}
