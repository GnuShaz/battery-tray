//! Мыши и клавиатуры Logitech на донглах Unifying, Bolt и Lightspeed по HID++ 2.0.
//! Читаем коллекцию приёмника 0xFF00 / usage 2 (длинные отчёты 0x11) и спрашиваем каждое из 6 мест.
//! ВНИМАНИЕ: код не запускался. Если батарея не находится, напишите, какой у вас приёмник (Unifying, Bolt или Lightspeed).
use crate::model::*;
use hidapi::{HidApi, HidDevice};

// Приёмник G Pro X Superlight 2 = 046D:C54D, сама мышь по USB = 046D:C09B (по патчам ядра Linux). Фильтруем по usage, не по PID.
const VENDOR: u16 = 0x046D;
const SWID: u8 = 0x0A;

pub struct LogitechSource;

/// Отправляет HID++ запрос и ждёт ответ с тем же функцией/swid. Формат: [0x11, устройство, фича, функция<<4|swid, params…].
fn request(dev: &HidDevice, index: u8, feature: u8, func: u8, params: &[u8]) -> Option<[u8; 20]> {
    let mut out = [0u8; 20];
    out[0] = 0x11;
    out[1] = index;
    out[2] = feature;
    out[3] = (func << 4) | SWID;
    out[4..4 + params.len()].copy_from_slice(params);
    dev.write(&out).ok()?;
    let mut buf = [0u8; 20];
    for _ in 0..8 {
        if dev.read_timeout(&mut buf, 300).ok()? == 0 { return None; }
        if buf[1] != index { continue; }
        if buf[2] == 0xFF { return None; } // устройство выключено или фичи нет
        if buf[2] == feature && buf[3] == out[3] { return Some(buf); }
    }
    None
}

/// IRoot.GetFeature: по ID фичи возвращает её индекс в таблице устройства.
fn feature_index(dev: &HidDevice, index: u8, id: u16) -> Option<u8> {
    let r = request(dev, index, 0x00, 0, &[(id >> 8) as u8, id as u8])?;
    (r[4] != 0).then_some(r[4])
}

/// Фичи 0x1004 (Unified Battery) и 0x1000 (Battery Status): процент в байте 0, статус зарядки в байте 2.
fn battery(dev: &HidDevice, index: u8) -> Option<(u8, bool)> {
    let (feature, func) = match feature_index(dev, index, 0x1004) {
        Some(f) => (f, 1),
        None => (feature_index(dev, index, 0x1000)?, 0),
    };
    let r = request(dev, index, feature, func, &[])?;
    Some((r[4], matches!(r[6], 1 | 2 | 4))) // 1, 2, 4: заряжается
}

/// Фича 0x0005: имя устройства кусками по 16 байт.
fn name(dev: &HidDevice, index: u8) -> Option<String> {
    let f = feature_index(dev, index, 0x0005)?;
    let count = request(dev, index, f, 0, &[])?[4] as usize;
    let mut bytes = Vec::new();
    while bytes.len() < count {
        let r = request(dev, index, f, 1, &[bytes.len() as u8])?;
        bytes.extend_from_slice(&r[4..4 + 16.min(count - bytes.len())]);
    }
    String::from_utf8(bytes).ok()
}

impl BatterySource for LogitechSource {
    fn poll(&mut self) -> Vec<Device> {
        let Ok(api) = HidApi::new() else { return vec![] };
        let mut out = Vec::new();
        for info in api.device_list().filter(|d| d.vendor_id() == VENDOR && d.usage_page() == 0xFF00 && d.usage() == 0x0002) {
            let Ok(dev) = info.open_device(&api) else { continue };
            for index in 1..=6u8 {
                let Some((percent, charging)) = battery(&dev, index) else { continue };
                let name = name(&dev, index).unwrap_or_else(|| "Logitech".into());
                out.push(Device { id: format!("logi:{name}"), kind: guess_kind(&name), name, link: Link::Dongle, percent: percent.min(100), charging });
            }
        }
        out
    }
}
