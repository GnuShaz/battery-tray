use crate::model::*;

/// Фейковые устройства из дизайна: удобно разрабатывать без железа (`BATTERY_TRAY_MOCK=1`).
pub struct MockSource { tick: u8 }
impl MockSource { pub fn new() -> Self { Self { tick: 0 } } }

impl BatterySource for MockSource {
    fn poll(&mut self) -> Vec<Device> {
        self.tick = self.tick.wrapping_add(1);
        let drain = (self.tick / 3).min(15);
        let d = |id: &str, name: &str, kind, link, percent: u8, charging| Device {
            id: id.into(), name: name.into(), kind, link, charging,
            percent: if charging { (percent + self.tick).min(100) } else { percent.saturating_sub(drain) },
        };
        vec![
            d("mouse", "MX Master 3S", Kind::Mouse, Link::Dongle, 82, false),
            d("kb", "Keychron K3", Kind::Keyboard, Link::Bluetooth, 64, false),
            d("hs", "WH-1000XM5", Kind::Headset, Link::Bluetooth, 18, false),
            d("pad", "Контроллер Xbox", Kind::Gamepad, Link::Dongle, 47, true),
        ]
    }
}
