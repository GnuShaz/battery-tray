//! Трей + всплывающее окно на Slint. Всё состояние живёт в главном потоке; из других потоков
//! (опрос, клики по трею, меню) данные приходят по каналу и разбираются таймером.
use crate::config::Config;
use crate::model::*;
use crate::render;
use crate::sources::{bluetooth::BluetoothSource, logitech::LogitechSource, mock::MockSource};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, Timer, TimerMode, VecModel};
use std::{cell::RefCell, collections::{HashMap, HashSet}, rc::Rc, sync::mpsc, time::{Duration, Instant}};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

slint::include_modules!();

enum Msg { Devices(Vec<Device>), Click { x: f64, y: f64, w: f64 }, Menu(MenuId) }

const POLL_EVERY: Duration = Duration::from_secs(60);
const POPUP_W: f64 = 384.0; // совпадает с ui/popup.slint

fn icon(pm: &tiny_skia::Pixmap) -> Icon {
    Icon::from_rgba(render::to_rgba(pm), render::SIZE, render::SIZE).expect("иконка")
}

fn hwnd(p: &TrayPopup) -> Option<isize> {
    let wh = p.window().window_handle();
    match wh.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

fn foreground() -> isize { unsafe { GetForegroundWindow().0 as isize } }

/// Окно не показываем на панели задач и сразу отдаём ему фокус, иначе автозакрытие не сработает.
fn tweak(h: isize) {
    unsafe {
        let h = HWND(h as *mut _);
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
        let _ = SetForegroundWindow(h);
    }
}

fn poll_loop(tx: mpsc::Sender<Msg>, wake: mpsc::Receiver<()>) {
    let mut sources: Vec<Box<dyn BatterySource>> = vec![Box::new(BluetoothSource), Box::new(LogitechSource)];
    if std::env::var_os("BATTERY_TRAY_MOCK").is_some() { sources.push(Box::new(MockSource::new())); }
    loop {
        let devices: Vec<Device> = sources.iter_mut().flat_map(|s| s.poll()).collect();
        if tx.send(Msg::Devices(devices)).is_err() { break; }
        let _ = wake.recv_timeout(POLL_EVERY); // кнопка «обновить» будит поток раньше срока
    }
}

struct App {
    popup: TrayPopup,
    menu: Menu,
    devices: Vec<Device>,
    cfg: Config,
    summary: Option<TrayIcon>,
    minis: HashMap<String, TrayIcon>,
    notified: HashSet<String>,
    visible: bool,
    shown_at: Instant,
    hidden_at: Instant,
}

impl App {
    fn refresh(&mut self) {
        let rows: Vec<Row> = self.devices.iter().map(|d| {
            let [r, g, b] = d.level().rgb();
            let link = if d.link == Link::Bluetooth { "Bluetooth" } else { "Донгл 2.4 ГГц" };
            let meta = match d.level() { Level::Charging => format!("{link}, заряжается"), Level::Low => format!("{link}, низкий заряд"), _ => link.to_string() };
            Row { name: d.name.clone().into(), meta: meta.into(), kind: d.kind as i32, percent: d.percent as i32,
                  color: slint::Color::from_rgb_u8(r, g, b), charging: d.charging, pinned: self.cfg.is_pinned(&d.id) }
        }).collect();
        self.popup.set_rows(ModelRc::new(VecModel::from(rows)));
        self.popup.set_percent_mode(self.cfg.percent_mode);
        self.sync_tray();
        self.notify_low();
    }

    fn sync_tray(&mut self) {
        // Кольцо: самое низкое значение среди тех, что не заряжаются.
        if let Some(low) = self.devices.iter().filter(|d| !d.charging).min_by_key(|d| d.percent) {
            let ic = icon(&render::summary(low.percent, low.level()));
            let tip = format!("{}: {}%", low.name, low.percent);
            match &self.summary {
                Some(t) => { let _ = t.set_icon(Some(ic)); let _ = t.set_tooltip(Some(tip)); }
                None => self.summary = TrayIconBuilder::new().with_icon(ic).with_tooltip(tip).with_menu(Box::new(self.menu.clone())).build().ok(),
            }
        }
        let style = if self.cfg.percent_mode { MiniStyle::Percent } else { MiniStyle::Bar };
        let pinned: Vec<&Device> = self.devices.iter().filter(|d| self.cfg.is_pinned(&d.id)).collect();
        self.minis.retain(|id, _| pinned.iter().any(|d| &d.id == id));
        for d in pinned {
            let ic = icon(&render::device(d.kind, d.percent, d.level(), style));
            let tip = format!("{}: {}%", d.name, d.percent);
            match self.minis.get(&d.id) {
                Some(t) => { let _ = t.set_icon(Some(ic)); let _ = t.set_tooltip(Some(tip)); }
                None => if let Ok(t) = TrayIconBuilder::new().with_icon(ic).with_tooltip(tip).build() { self.minis.insert(d.id.clone(), t); },
            }
        }
    }

    /// Одно уведомление на каждое падение ниже 20%; сбрасывается, когда заряд выше или устройство на зарядке.
    fn notify_low(&mut self) {
        for d in &self.devices {
            if d.level() == Level::Low {
                if self.notified.insert(d.id.clone()) {
                    let _ = notify_rust::Notification::new().appname("battery-tray").summary("Низкий заряд")
                        .body(&format!("{}: {}%", d.name, d.percent)).show();
                }
            } else { self.notified.remove(&d.id); }
        }
    }

    fn toggle_pin(&mut self, i: usize) {
        let Some(id) = self.devices.get(i).map(|d| d.id.clone()) else { return };
        let all: Vec<String> = self.devices.iter().map(|d| d.id.clone()).collect();
        self.cfg.toggle_pin(&id, &all);
        self.cfg.save();
        self.refresh();
    }

    fn set_percent_mode(&mut self, on: bool) {
        self.cfg.percent_mode = on;
        self.cfg.save();
        self.refresh();
    }

    fn show(&mut self, x: f64, y: f64, w: f64) {
        let p = &self.popup;
        let scale = p.window().scale_factor() as f64;
        let height = (104.0 + 82.0 * self.devices.len() as f64 + 48.0) * scale;
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) } as f64;
        let px = (x + w / 2.0 - POPUP_W * scale / 2.0).clamp(8.0, (screen_w - POPUP_W * scale - 8.0).max(8.0));
        p.set_shown(false);
        let _ = p.show();
        p.window().set_position(PhysicalPosition::new(px as i32, (y - height) as i32)); // низ окна у верха иконки
        if let Some(h) = hwnd(p) { tweak(h); }
        let weak = p.as_weak();
        Timer::single_shot(Duration::from_millis(40), move || { if let Some(p) = weak.upgrade() { p.set_shown(true); } });
        self.visible = true;
        self.shown_at = Instant::now();
    }

    fn hide(&mut self) {
        if !self.visible { return; }
        self.visible = false;
        self.hidden_at = Instant::now();
        self.popup.set_shown(false);
        let weak = self.popup.as_weak();
        Timer::single_shot(Duration::from_millis(220), move || { if let Some(p) = weak.upgrade() { let _ = p.hide(); } });
    }
}

pub fn run() {
    let (tx, rx) = mpsc::channel::<Msg>();
    let (wake_tx, wake_rx) = mpsc::channel::<()>();

    let autostart = std::env::current_exe().ok().and_then(|p| {
        auto_launch::AutoLaunchBuilder::new().set_app_name("battery-tray").set_app_path(&p.to_string_lossy()).build().ok()
    });
    let menu = Menu::new();
    let auto_item = CheckMenuItem::new("Запускать с Windows", true, autostart.as_ref().and_then(|a| a.is_enabled().ok()).unwrap_or(false), None);
    let quit = MenuItem::new("Выход", true, None);
    menu.append_items(&[&auto_item, &quit]).unwrap();
    let (auto_id, quit_id) = (auto_item.id().clone(), quit.id().clone());

    { let tx = tx.clone(); MenuEvent::set_event_handler(Some(move |e: MenuEvent| { let _ = tx.send(Msg::Menu(e.id)); })); }
    { let tx = tx.clone(); TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { rect, button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            let _ = tx.send(Msg::Click { x: rect.position.x, y: rect.position.y, w: rect.size.width as f64 });
        }
    })); }
    std::thread::spawn(move || poll_loop(tx, wake_rx));

    let popup = TrayPopup::new().unwrap();
    let app = Rc::new(RefCell::new(App {
        popup: popup.clone_strong(), menu, devices: vec![], cfg: Config::load(), summary: None, minis: HashMap::new(),
        notified: HashSet::new(), visible: false, shown_at: Instant::now(), hidden_at: Instant::now(),
    }));

    { let a = app.clone(); popup.on_pin(move |i| a.borrow_mut().toggle_pin(i as usize)); }
    { let a = app.clone(); popup.on_set_percent_mode(move |v| a.borrow_mut().set_percent_mode(v)); }
    { let a = app.clone(); popup.on_dismiss(move || a.borrow_mut().hide()); }
    { let a = app.clone(); popup.on_refresh(move || {
        let p = a.borrow().popup.clone_strong();
        p.set_spin_count(p.get_spin_count() + 1);
        let _ = wake_tx.send(());
    }); }

    let timer = Timer::default();
    {
        let app = app.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
            let mut a = app.borrow_mut();
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Msg::Devices(d) => { a.devices = d; a.refresh(); }
                    Msg::Click { x, y, w } => {
                        if a.visible { a.hide(); }
                        // клик по иконке сначала отнимает фокус и закрывает окно, второй раз его открывать не нужно
                        else if a.hidden_at.elapsed() > Duration::from_millis(300) { a.show(x, y, w); }
                    }
                    Msg::Menu(id) if id == quit_id => { let _ = slint::quit_event_loop(); }
                    Msg::Menu(id) if id == auto_id => {
                        if let Some(al) = &autostart { let _ = if auto_item.is_checked() { al.enable() } else { al.disable() }; }
                    }
                    Msg::Menu(_) => {}
                }
            }
            // закрываем, когда фокус ушёл из окна
            if a.visible && a.shown_at.elapsed() > Duration::from_millis(400) {
                if let Some(h) = hwnd(&a.popup) { if foreground() != h { a.hide(); } }
            }
        });
    }
    let _ = slint::run_event_loop_until_quit();
}
