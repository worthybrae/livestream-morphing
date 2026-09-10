use std::collections::VecDeque;

pub struct Segment {
    pub id: String,
    pub data: Vec<u8>,
    pub duration: f32,
    pub raw: Option<Vec<u8>>,
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
        self.push_timed_segment(id, data, 6.0);
    }

    pub fn push_timed_segment(&mut self, id: String, data: Vec<u8>, duration: f32) {
        self.push_paired_segment(id, data, duration, None);
    }

    pub fn push_paired_segment(&mut self, id: String, data: Vec<u8>, duration: f32, raw: Option<Vec<u8>>) {
        if self.segments.len() >= self.max_segments {
            self.segments.pop_front();
            self.sequence += 1;
        }
        self.segments.push_back(Segment {
            id,
            data,
            duration,
            raw,
        });
    }

    pub fn get_segment(&self, id: &str) -> Option<&[u8]> {
        self.segments
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.data.as_slice())
    }

    pub fn get_raw_segment(&self, id: &str) -> Option<&[u8]> {
        self.segments.iter().find(|s| s.id == id).and_then(|s| s.raw.as_deref())
    }

    pub fn generate_raw_playlist(&self) -> String {
        // Publish only paired segments. Each source segment has its own timestamp
        // domain, so discontinuities also cover source gaps without inventing frames.
        let mut playlist = format!("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:13\n#EXT-X-MEDIA-SEQUENCE:{}\n#EXT-X-DISCONTINUITY-SEQUENCE:{}\n", self.sequence, self.sequence);
        for seg in &self.segments {
            if seg.raw.is_none() { return "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:13\n".into(); }
            playlist.push_str(&format!("#EXT-X-DISCONTINUITY\n#EXTINF:{:.3},\n/api/raw-segments/{}.ts\n", seg.duration, seg.id));
        }
        playlist
    }

    pub fn generate_playlist(&self) -> String {
        // Fixed for Abbey Road, whose source advertises a 13-second upper bound.
        let target = 13;
        let mut m3u8 = format!(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:{}\n",
            self.sequence
        );
        for seg in &self.segments {
            m3u8.push_str(&format!(
                "#EXTINF:{:.3},\n/api/segments/{}.ts\n",
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

    /// Return segment IDs and sizes for the status API.
    pub fn segment_info(&self) -> Vec<(String, usize)> {
        self.segments
            .iter()
            .map(|s| (s.id.clone(), s.data.len()))
            .collect()
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
    fn paired_source_is_published_and_evicted_with_its_drawing() {
        let mut buffer = HlsBuffer::new(1);
        buffer.push_paired_segment("one".into(), vec![1], 6.006, Some(vec![2]));
        assert_eq!(buffer.get_raw_segment("one"), Some([2u8].as_slice()));
        assert!(buffer.generate_raw_playlist().contains("#EXTINF:6.006,\n/api/raw-segments/one.ts"));
        buffer.push_paired_segment("two".into(), vec![3], 6.006, Some(vec![4]));
        assert!(buffer.get_segment("one").is_none());
        assert!(buffer.get_raw_segment("one").is_none());
        assert!(buffer.generate_raw_playlist().contains("#EXT-X-DISCONTINUITY-SEQUENCE:1"));
        buffer.clear();
        assert!(buffer.get_raw_segment("two").is_none());
    }

    #[test]
    fn fractional_source_duration_is_preserved() {
        let mut buffer=HlsBuffer::new(10);
        buffer.push_timed_segment("source".into(),vec![],6.006);
        let playlist=buffer.generate_playlist();
        assert!(playlist.contains("#EXTINF:6.006,"));
        assert!(playlist.contains("#EXT-X-TARGETDURATION:13"));
    }
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
