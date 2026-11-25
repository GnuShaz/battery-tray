#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Mouse, Keyboard, Headset, Gamepad, Other }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link { Bluetooth, Dongle }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level { Normal, Charging, Low }

impl Level {
    /// Цвета те же, что в дизайне.
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Level::Normal => [0x8A, 0xB4, 0xFF],
            Level::Charging => [0x4F, 0xD1, 0xA1],
            Level::Low => [0xFF, 0x8F, 0x5E],
        }
    }
    pub fn hex(self) -> String {
        let [r, g, b] = self.rgb();
        format!("#{r:02X}{g:02X}{b:02X}")
    }
}

#[derive(Clone, Debug)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub link: Link,
    pub percent: u8,
    pub charging: bool,
}

impl Device {
    pub fn level(&self) -> Level {
        if self.charging { Level::Charging }
        else if self.percent < 20 { Level::Low }
        else { Level::Normal }
    }
}

/// Источник данных о заряде: Bluetooth, донгл конкретного бренда и т. д.
pub trait BatterySource: Send {
    fn poll(&mut self) -> Vec<Device>;
}

pub fn guess_kind(name: &str) -> Kind {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["mouse", "mx "]) { Kind::Mouse }
    else if has(&["keyboard", "keychron", "k380"]) { Kind::Keyboard }
    else if has(&["headphone", "headset", "buds", "wh-", "airpods"]) { Kind::Headset }
    else if has(&["controller", "gamepad", "xbox", "dualsense"]) { Kind::Gamepad }
    else { Kind::Other }
}

/// Что показывать под значком в мини-иконке трея.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MiniStyle { Bar, Percent }
