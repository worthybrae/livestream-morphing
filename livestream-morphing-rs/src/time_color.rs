// London time → color palette
use chrono::Timelike;
use chrono::Utc;
use chrono_tz::Europe::London;

pub type Color = (u8, u8, u8);

/// Maps hour+minute to a grey level (25=darkest at midnight, 175=lightest at noon).
pub fn get_grey_level(hour: u32, minute: u32) -> u8 {
    let total_minutes = hour * 60 + minute;
    let distance_from_noon = if total_minutes > 720 {
        1440 - total_minutes
    } else {
        total_minutes
    };
    let lightest: u8 = 175;
    let darkest: u8 = 25;
    let range = (lightest - darkest) as f32;
    darkest + (range * distance_from_noon as f32 / 720.0) as u8
}

/// Returns (edge_color, background_color) based on time of day.
pub fn get_colors(hour: u32, minute: u32) -> (Color, Color) {
    let grey = get_grey_level(hour, minute);
    let bg = (grey, grey, grey);
    let edge = if grey > 127 { (0, 0, 0) } else { (255, 255, 255) };
    (edge, bg)
}

/// Gets colors for the current London time.
pub fn get_colors_now() -> (Color, Color) {
    let london_now = Utc::now().with_timezone(&London);
    get_colors(london_now.hour(), london_now.minute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_lightest() {
        assert_eq!(get_grey_level(12, 0), 175);
    }

    #[test]
    fn midnight_is_darkest() {
        assert_eq!(get_grey_level(0, 0), 25);
    }

    #[test]
    fn six_am_is_midpoint() {
        let level = get_grey_level(6, 0);
        assert!(level > 90 && level < 110, "6am should be mid-grey, got {level}");
    }

    #[test]
    fn six_pm_mirrors_six_am() {
        assert_eq!(get_grey_level(6, 0), get_grey_level(18, 0));
    }

    #[test]
    fn light_background_gets_dark_edges() {
        let (edge, bg) = get_colors(12, 0);
        assert!(bg.0 > 127);
        assert_eq!(edge, (0, 0, 0));
    }

    #[test]
    fn dark_background_gets_light_edges() {
        let (edge, bg) = get_colors(0, 0);
        assert!(bg.0 <= 127);
        assert_eq!(edge, (255, 255, 255));
    }
}
