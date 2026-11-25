//! Диагностика для неизвестных донглов: `battery-tray.exe --dump-hid [VID:PID]` пишет hid-dump.txt в текущую папку.
//! Без аргумента: список всех HID-интерфейсов. С VID:PID (hex): ещё читает feature-отчёты и слушает входящие 20 секунд.
use hidapi::HidApi;
use std::{fmt::Write as _, time::{Duration, Instant}};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ")
}

pub fn parse_id(s: &str) -> Option<(u16, u16)> {
    let (v, p) = s.split_once(':')?;
    Some((u16::from_str_radix(v, 16).ok()?, u16::from_str_radix(p, 16).ok()?))
}

pub fn run(filter: Option<(u16, u16)>) {
    let Ok(api) = HidApi::new() else { return };
    let mut out = String::new();
    for d in api.device_list() {
        let _ = writeln!(out, "{:04X}:{:04X} if={} usage={:04X}/{:04X} {} | {}", d.vendor_id(), d.product_id(),
            d.interface_number(), d.usage_page(), d.usage(),
            d.manufacturer_string().unwrap_or("?"), d.product_string().unwrap_or("?"));
    }
    if let Some((vid, pid)) = filter {
        let mut open = Vec::new();
        for d in api.device_list().filter(|d| d.vendor_id() == vid && d.product_id() == pid) {
            let Ok(h) = d.open_device(&api) else {
                let _ = writeln!(out, "не открылся: if={} usage={:04X}/{:04X}", d.interface_number(), d.usage_page(), d.usage());
                continue;
            };
            for id in 0u8..=16 {
                let mut buf = [0u8; 65];
                buf[0] = id;
                if let Ok(n) = h.get_feature_report(&mut buf) {
                    let _ = writeln!(out, "feature if={} id={id}: {}", d.interface_number(), hex(&buf[..n]));
                }
            }
            let _ = h.set_blocking_mode(false);
            open.push((d.interface_number(), d.usage_page(), d.usage(), h));
        }
        let _ = writeln!(out, "слушаю 20 с: нажимайте клавиши, Fn-комбинации, подключите кабель зарядки");
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            for (i, page, usage, h) in &open {
                let mut buf = [0u8; 65];
                if let Ok(n) = h.read(&mut buf) {
                    if n > 0 { let _ = writeln!(out, "input if={i} usage={page:04X}/{usage:04X}: {}", hex(&buf[..n])); }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let _ = std::fs::write("hid-dump.txt", out);
}
