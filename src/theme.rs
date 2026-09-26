use crate::config::home;
use std::path::PathBuf;

/// Slot 0 = background, 1..=6 = foreground ramp (dark → bright), 7 = highlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub colors: [[u8; 3]; 8],
}

fn current_dir() -> PathBuf {
    home().join(".local/state/omarchy/current")
}

pub fn theme_name() -> String {
    std::fs::read_to_string(current_dir().join("theme.name"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim();
    let h = s.strip_prefix('#').or_else(|| s.strip_prefix("0x")).unwrap_or(s);
    if h.len() < 6 {
        return None;
    }
    let v = u32::from_str_radix(&h[..6], 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn luminance(c: [u8; 3]) -> f32 {
    0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32
}

impl Palette {
    pub fn from_toml(src: &str) -> Palette {
        let table: toml::Table = src.parse().unwrap_or_default();
        let get = |k: &str| table.get(k).and_then(|v| v.as_str()).and_then(parse_hex);
        let fg = get("foreground").unwrap_or([255, 255, 255]);
        let mut colors = [[0u8; 3]; 8];
        colors[0] = get("background").unwrap_or([0, 0, 0]);
        let mut ramp = ["dark_foreground", "blue", "cyan", "accent", "magenta", "foreground"]
            .map(|k| get(k).unwrap_or(fg));
        ramp.sort_by(|a, b| luminance(*a).total_cmp(&luminance(*b)));
        colors[1..7].copy_from_slice(&ramp);
        colors[7] = get("light_foreground").unwrap_or(fg);
        Palette { colors }
    }

    /// Current Omarchy theme palette; a missing colors.toml yields the fallback palette.
    pub fn load() -> Palette {
        let src = std::fs::read_to_string(current_dir().join("theme/colors.toml")).unwrap_or_default();
        Palette::from_toml(&src)
    }

    /// Nearest foreground slot (1..=7) by squared RGB distance.
    pub fn nearest(&self, rgb: [u8; 3]) -> u8 {
        let dist = |c: [u8; 3]| -> i32 {
            (0..3).map(|i| (c[i] as i32 - rgb[i] as i32).pow(2)).sum()
        };
        (1..8u8).min_by_key(|&i| dist(self.colors[i as usize])).unwrap()
    }
}

/// Map a value in [0,1] to foreground ramp slot 1..=6.
#[inline]
pub fn value_color(v: f32) -> u8 {
    1 + ((v.clamp(0.0, 1.0) * 6.0) as u8).min(5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_fall_back_and_ramp_sorted() {
        let p = Palette::from_toml("foreground = \"#808080\"\nblue = \"#000010\"\ncyan = \"#ffffff\"");
        assert_eq!(p.colors[0], [0, 0, 0]);
        assert_eq!(p.colors[7], [0x80, 0x80, 0x80]);
        assert_eq!(p.colors[1], [0, 0, 0x10]);
        assert_eq!(p.colors[6], [0xff, 0xff, 0xff]);
        for w in p.colors[1..7].windows(2) {
            assert!(luminance(w[0]) <= luminance(w[1]));
        }
        let empty = Palette::from_toml("");
        assert!(empty.colors[1..8].iter().all(|c| *c == [255, 255, 255]));
    }

    #[test]
    fn value_color_bounds() {
        assert_eq!(value_color(0.0), 1);
        assert_eq!(value_color(1.0), 6);
        assert_eq!(value_color(-3.0), 1);
        assert_eq!(value_color(0.5), 4);
    }
}
