#![cfg_attr(not(windows), allow(dead_code))]
//! "Back to Spine" from the controller while a game runs.
//!
//! While an emulator is in front, Spine's window can't see the controller, so this
//! reads Xbox-style controllers directly (XInput, which Windows ships) and watches
//! for Select + Start held together. When it sees that, it asks the emulator to
//! close like clicking its X would (so games save normally), and only forces it
//! closed if it hasn't gone after a few seconds.
//!
//! PlayStation / Switch pads work too when Steam Input or DS4Windows presents them
//! as Xbox controllers, which is how most people use them on PC.

#[cfg_attr(not(windows), allow(unused_imports))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const BACK: u16 = 0x0020;
pub const START: u16 = 0x0010;

/// How long the combo must be held, so a quick press in a game's menu doesn't quit.
pub const HOLD: Duration = Duration::from_millis(1200);

/// Tracks the combo across polls: returns true once it has been held for `hold`.
pub struct Combo {
    since: Option<Instant>,
    hold: Duration,
}

impl Combo {
    pub fn new(hold: Duration) -> Self {
        Combo { since: None, hold }
    }
    pub fn update(&mut self, buttons: u16, now: Instant) -> bool {
        if buttons & (BACK | START) == (BACK | START) {
            let s = *self.since.get_or_insert(now);
            now.duration_since(s) >= self.hold
        } else {
            self.since = None;
            false
        }
    }
}

#[cfg(windows)]
mod xinput {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct Gamepad {
        pub buttons: u16,
        pub left_trigger: u8,
        pub right_trigger: u8,
        pub lx: i16,
        pub ly: i16,
        pub rx: i16,
        pub ry: i16,
    }
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct State {
        pub packet: u32,
        pub pad: Gamepad,
    }

    type GetState = unsafe extern "system" fn(u32, *mut State) -> u32;

    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    pub struct XInput {
        get: GetState,
    }

    impl XInput {
        pub fn load() -> Option<XInput> {
            const DLLS: [&[u8]; 3] = [b"xinput1_4.dll\0", b"xinput1_3.dll\0", b"xinput9_1_0.dll\0"];
            for dll in DLLS {
                unsafe {
                    let m = LoadLibraryA(dll.as_ptr());
                    if m.is_null() {
                        continue;
                    }
                    let f = GetProcAddress(m, b"XInputGetState\0".as_ptr());
                    if !f.is_null() {
                        return Some(XInput { get: std::mem::transmute::<*mut c_void, GetState>(f) });
                    }
                }
            }
            None
        }
        /// Buttons held on any of the four controller slots, combined.
        pub fn buttons(&self) -> u16 {
            let mut all = 0;
            for i in 0..4 {
                let mut s = State::default();
                if unsafe { (self.get)(i, &mut s) } == 0 {
                    all |= s.pad.buttons;
                }
            }
            all
        }
    }
}

/// Linux: controllers through the kernel's input devices. Desktop Linux lets the
/// signed-in user read game controllers (not keyboards), and every controller
/// driver (Xbox, PlayStation, Switch Pro, 8BitDo...) reports Select/Back and
/// Start with the same two codes, so no button mapping is needed.
#[cfg(target_os = "linux")]
mod evdev {
    use super::{BACK, START};
    use std::fs::File;
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    use std::time::{Duration, Instant};

    const EV_KEY: u16 = 1;
    const BTN_SELECT: u16 = 0x13a;
    const BTN_START: u16 = 0x13b;
    const O_NONBLOCK: i32 = 0o4000;
    /// struct input_event on 64-bit Linux: 16-byte time, u16 type, u16 code, i32 value.
    const EVENT: usize = 24;

    pub struct Pads {
        devs: Vec<(std::path::PathBuf, File, u16)>,
        scanned: Instant,
    }

    /// Turn raw events into held buttons (BACK/START bits).
    pub fn apply(mut held: u16, buf: &[u8]) -> u16 {
        for ev in buf.chunks_exact(EVENT) {
            let ty = u16::from_ne_bytes([ev[16], ev[17]]);
            let code = u16::from_ne_bytes([ev[18], ev[19]]);
            let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
            if ty != EV_KEY {
                continue;
            }
            let bit = match code {
                BTN_SELECT => BACK,
                BTN_START => START,
                _ => continue,
            };
            if value != 0 { held |= bit } else { held &= !bit }
        }
        held
    }

    impl Pads {
        pub fn open() -> Self {
            let mut p = Pads { devs: vec![], scanned: Instant::now() };
            p.rescan();
            p
        }
        fn rescan(&mut self) {
            self.scanned = Instant::now();
            let Ok(rd) = std::fs::read_dir("/dev/input") else { return };
            for e in rd.flatten() {
                let path = e.path();
                if !e.file_name().to_string_lossy().starts_with("event") || self.devs.iter().any(|(p, _, _)| *p == path) {
                    continue;
                }
                if let Ok(f) = std::fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(&path) {
                    self.devs.push((path, f, 0));
                }
            }
        }
        /// Buttons held on any controller, combined (controllers plugged in later are found too).
        pub fn buttons(&mut self) -> u16 {
            if self.scanned.elapsed() > Duration::from_secs(3) {
                self.rescan();
            }
            let mut all = 0;
            let mut gone = vec![];
            for (i, (_, f, held)) in self.devs.iter_mut().enumerate() {
                let mut buf = [0u8; EVENT * 32];
                loop {
                    match f.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => *held = apply(*held, &buf[..n - n % EVENT]),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(_) => {
                            gone.push(i); // unplugged
                            break;
                        }
                    }
                }
                all |= *held;
            }
            for i in gone.into_iter().rev() {
                self.devs.remove(i);
            }
            all
        }
    }
}

/// Close a process tree the polite way first (like clicking X), then for good.
pub fn close_game(pid: u32, still_running: &dyn Fn() -> bool) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let run = |force: bool| {
            let mut c = std::process::Command::new("taskkill");
            c.args(["/PID", &pid.to_string(), "/T"]);
            if force {
                c.arg("/F");
            }
            let _ = c.creation_flags(0x0800_0000).output();
        };
        run(false);
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if !still_running() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        run(true);
    }
    #[cfg(not(windows))]
    {
        // Games start in their own process group, so this reaches everything they started.
        let group = format!("-{pid}");
        let _ = spine_core::hostenv::command("kill").args(["-TERM", "--", &group]).status();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) && still_running() {
            std::thread::sleep(Duration::from_millis(200));
        }
        if still_running() {
            let _ = spine_core::hostenv::command("kill").args(["-KILL", "--", &group]).status();
        }
    }
}

/// Watch the controllers until `done` is set; call `on_combo` once if the combo is held.
pub fn watch(done: Arc<AtomicBool>, on_combo: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        #[cfg(windows)]
        {
            let Some(x) = xinput::XInput::load() else { return };
            let mut combo = Combo::new(HOLD);
            // Ignore buttons already held when the game started (e.g. Start used to launch).
            while !done.load(Ordering::Relaxed) && x.buttons() & (BACK | START) != 0 {
                std::thread::sleep(Duration::from_millis(50));
            }
            while !done.load(Ordering::Relaxed) {
                if combo.update(x.buttons(), Instant::now()) {
                    on_combo();
                    return;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        }
        #[cfg(target_os = "linux")]
        {
            let mut pads = evdev::Pads::open();
            let mut combo = Combo::new(HOLD);
            while !done.load(Ordering::Relaxed) && pads.buttons() & (BACK | START) != 0 {
                std::thread::sleep(Duration::from_millis(50));
            }
            while !done.load(Ordering::Relaxed) {
                if combo.update(pads.buttons(), Instant::now()) {
                    on_combo();
                    return;
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = (done, on_combo);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_input_events() {
        let ev = |code: u16, value: i32| {
            let mut b = vec![0u8; 16];
            b.extend(1u16.to_ne_bytes());
            b.extend(code.to_ne_bytes());
            b.extend(value.to_ne_bytes());
            b
        };
        let mut buf = ev(0x13a, 1);
        buf.extend(ev(0x13b, 1));
        buf.extend(ev(0x130, 1)); // A: ignored
        assert_eq!(evdev::apply(0, &buf), BACK | START);
        assert_eq!(evdev::apply(BACK | START, &ev(0x13a, 0)), START);
    }

    #[test]
    fn combo_needs_both_buttons_held() {
        let t0 = Instant::now();
        let mut c = Combo::new(HOLD);
        assert!(!c.update(BACK | START, t0));
        assert!(!c.update(BACK | START, t0 + Duration::from_millis(600)));
        // Let go of one: starts over.
        assert!(!c.update(START, t0 + Duration::from_millis(700)));
        assert!(!c.update(BACK | START | 0x1000, t0 + Duration::from_millis(800)));
        assert!(!c.update(BACK | START, t0 + Duration::from_millis(1900)));
        assert!(c.update(BACK | START, t0 + Duration::from_millis(2100)));
        assert!(!Combo::new(HOLD).update(BACK, t0));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod close_tests {
    use super::*;
    use std::os::unix::process::CommandExt;

    #[test]
    fn closes_the_whole_game() {
        // A "game" that starts a helper of its own, like flatpak run does.
        let mut child = std::process::Command::new("sh").args(["-c", "sleep 300 & echo $! > /tmp/spine-close-test.pid; wait"]).process_group(0).spawn().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let helper: u32 = std::fs::read_to_string("/tmp/spine-close-test.pid").unwrap().trim().parse().unwrap();
        let pid = child.id();
        let start = Instant::now();
        close_game(pid, &|| std::path::Path::new(&format!("/proc/{pid}")).exists() && !std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default().contains(") Z"));
        let _ = child.wait();
        assert!(start.elapsed() < Duration::from_secs(4), "closed politely, no 5 s wait");
        std::thread::sleep(Duration::from_millis(200));
        let alive = std::fs::read_to_string(format!("/proc/{helper}/stat")).map(|s| !s.contains(") Z")).unwrap_or(false);
        assert!(!alive, "the game's helper process was closed too");
    }
}
