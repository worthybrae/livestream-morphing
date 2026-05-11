use reqwest::Client;

const DEFAULT_STREAM_URL: &str =
    "https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w";

fn earthcam_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Origin", "https://www.abbeyroad.com".parse().unwrap());
    headers.insert("Referer", "https://www.abbeyroad.com/".parse().unwrap());
    headers.insert(
        "User-Agent",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
            .parse()
            .unwrap(),
    );
    headers
}

/// Extract the segment ID from the latest .ts URI in an M3U8 playlist.
/// URI format: `media_w{timestamp}_{segment_id}.ts`
pub fn extract_segment_id(m3u8_text: &str) -> Option<String> {
    m3u8_text
        .lines()
        .filter(|line| line.ends_with(".ts"))
        .last()
        .and_then(|line| {
            let name = line.trim();
            let without_ext = name.strip_suffix(".ts")?;
            let id = without_ext.rsplit('_').next()?;
            Some(id.to_string())
        })
}

pub struct StreamSource {
    client: Client,
    base_url: String,
    recent_ids: Vec<String>,
}

impl StreamSource {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            recent_ids: Vec::new(),
        }
    }

    pub fn default_url() -> String {
        std::env::var("STREAM_URL")
            .unwrap_or_else(|_| DEFAULT_STREAM_URL.to_string())
    }

    /// Fetch the latest segment ID from the Abbey Road stream.
    /// Returns `None` if the segment was already seen or fetch fails.
    pub async fn fetch_latest_segment_id(&mut self) -> Option<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let url = format!("{}{timestamp}.m3u8", self.base_url);

        let resp = self
            .client
            .get(&url)
            .headers(earthcam_headers())
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .ok()?;

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "Playlist fetch failed");
            return None;
        }

        let text = resp.text().await.ok()?;
        let id = extract_segment_id(&text)?;

        if self.recent_ids.contains(&id) {
            return None;
        }

        self.recent_ids.push(id.clone());
        if self.recent_ids.len() > 20 {
            self.recent_ids.remove(0);
        }

        Some(id)
    }

    /// Download a .ts segment by ID. Retries up to 3 times.
    pub async fn download_segment(&self, segment_id: &str) -> Option<Vec<u8>> {
        let base_url = self.base_url.replace("/chunklist_w", "/media_w");

        for attempt in 0..3 {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let url = format!("{base_url}{timestamp}_{segment_id}.ts");

            match self
                .client
                .get(&url)
                .headers(earthcam_headers())
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(bytes) = resp.bytes().await {
                        tracing::info!(
                            segment_id,
                            size_mb = bytes.len() as f64 / 1_048_576.0,
                            "Downloaded segment"
                        );
                        return Some(bytes.to_vec());
                    }
                }
                Ok(resp) => {
                    tracing::warn!(segment_id, attempt, status = %resp.status(), "Download failed");
                }
                Err(e) => {
                    tracing::warn!(segment_id, attempt, error = %e, "Download error");
                }
            }

            if attempt < 2 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
        }

        tracing::error!(segment_id, "Failed to download after 3 attempts");
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_segment_id_from_m3u8() {
        let m3u8 = "\
#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:54321
#EXTINF:6.006,
media_w1715000000_99887.ts
";
        let id = extract_segment_id(m3u8);
        assert_eq!(id, Some("99887".to_string()));
    }

    #[test]
    fn parse_returns_none_for_empty_playlist() {
        let m3u8 = "#EXTM3U\n#EXT-X-VERSION:3\n";
        assert_eq!(extract_segment_id(m3u8), None);
    }
}
