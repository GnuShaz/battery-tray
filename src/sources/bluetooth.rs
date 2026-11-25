//! Заряд Bluetooth-устройств из свойства PnP DEVPKEY_Bluetooth_Battery
//! ({104EA319-6EE2-4701-BD47-8DDBF425BBE5}, pid 2). Его же показывает «Параметры Windows».
//! ВНИМАНИЕ: код не компилировался (писался без Windows), возможны правки под версию крейта `windows`.
use crate::model::*;
use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Properties::DEVPROPKEY;

const BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5),
    pid: 2,
};
const FRIENDLY_NAME: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0xA45C254E_DF1C_4EFD_8020_67D146A850E0),
    pid: 14,
};

pub struct BluetoothSource;

impl BatterySource for BluetoothSource {
    fn poll(&mut self) -> Vec<Device> {
        let mut out = Vec::new();
        // BTHENUM — классический Bluetooth, BTHLE — Low Energy.
        for enumerator in [w!("BTHENUM"), w!("BTHLE")] {
            unsafe { collect(enumerator, &mut out) };
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out.dedup_by(|a, b| a.id == b.id);
        out
    }
}

unsafe fn collect(enumerator: PCWSTR, out: &mut Vec<Device>) {
    let Ok(set) = SetupDiGetClassDevsW(None, enumerator, None, DIGCF_PRESENT | DIGCF_ALLCLASSES) else { return };
    let mut index = 0;
    loop {
        let mut info = SP_DEVINFO_DATA { cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
        if SetupDiEnumDeviceInfo(set, index, &mut info).is_err() { break; }
        index += 1;

        let mut ty = Default::default();
        let mut battery = [0u8; 1];
        // Свойства нет у устройств без датчика заряда, их пропускаем.
        if SetupDiGetDevicePropertyW(set, &info, &BATTERY, &mut ty, Some(&mut battery), None, 0).is_err() { continue; }

        let mut buf = [0u8; 512];
        let name = if SetupDiGetDevicePropertyW(set, &info, &FRIENDLY_NAME, &mut ty, Some(&mut buf), None, 0).is_ok() {
            let wide: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
            String::from_utf16_lossy(&wide)
        } else { "Bluetooth".to_string() };

        out.push(Device {
            id: format!("bt:{name}"),
            kind: guess_kind(&name),
            name,
            link: Link::Bluetooth,
            percent: battery[0].min(100),
            charging: false, // TODO: Windows не отдаёт статус зарядки через это свойство
        });
    }
    let _ = SetupDiDestroyDeviceInfoList(set);
}
