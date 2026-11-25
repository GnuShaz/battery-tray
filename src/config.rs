//! Настройки в %APPDATA%\battery-tray\config.txt. Без serde: два вида строк, `percent=1` и `pin=<id>`.
use std::{collections::HashSet, path::PathBuf};

#[derive(Default, Debug, PartialEq)]
pub struct Config {
    /// None = первый запуск, закреплены все устройства.
    pub pins: Option<HashSet<String>>,
    pub percent_mode: bool,
}

impl Config {
    pub fn is_pinned(&self, id: &str) -> bool {
        self.pins.as_ref().map_or(true, |p| p.contains(id))
    }

    pub fn toggle_pin(&mut self, id: &str, all: &[String]) {
        let set = self.pins.get_or_insert_with(|| all.iter().cloned().collect());
        if !set.remove(id) { set.insert(id.to_string()); }
    }

    pub fn parse(text: &str) -> Self {
        let mut cfg = Config::default();
        for line in text.lines() {
            match line.split_once('=') {
                Some(("percent", v)) => cfg.percent_mode = v.trim() == "1",
                Some(("pins", _)) => { cfg.pins.get_or_insert_with(HashSet::new); }
                Some(("pin", id)) => { cfg.pins.get_or_insert_with(HashSet::new).insert(id.to_string()); }
                _ => {}
            }
        }
        cfg
    }

    pub fn to_text(&self) -> String {
        let mut out = format!("percent={}\n", self.percent_mode as u8);
        if let Some(pins) = &self.pins {
            out.push_str("pins=custom\n");
            let mut ids: Vec<_> = pins.iter().collect();
            ids.sort();
            for id in ids { out.push_str(&format!("pin={id}\n")); }
        }
        out
    }

    fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("battery-tray").join("config.txt")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path()).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
        let _ = std::fs::write(path, self.to_text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut c = Config::default();
        assert!(c.is_pinned("a"));
        c.toggle_pin("a", &["a".into(), "b".into()]); // открепили a, остался b
        c.percent_mode = true;
        let back = Config::parse(&c.to_text());
        assert_eq!(c, back);
        assert!(!back.is_pinned("a") && back.is_pinned("b"));
    }
}
