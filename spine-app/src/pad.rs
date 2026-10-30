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
        let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) && still_running() {
            std::thread::sleep(Duration::from_millis(200));
        }
        if still_running() {
            let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
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
        #[cfg(not(windows))]
        {
            let _ = (done, on_combo);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
