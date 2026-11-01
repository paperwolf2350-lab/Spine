//! Where you left off in a movie or episode, and whether you've watched it.
//!
//! VLC is started with its web remote turned on, listening only on this PC
//! (127.0.0.1) with a random password. While it plays, Spine asks it every few
//! seconds how far in you are; when it closes, the last answer is saved.
//!
//! mpv is started with a small script of Spine's that writes "<seconds> <length>"
//! to a file every couple of seconds and once more as the video closes; Spine
//! reads that file the same way.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Saved per movie, or per episode file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Seconds in, where to resume (0 once watched).
    #[serde(default)]
    pub pos: u64,
    /// Length in seconds, as the player reported it (0 = unknown).
    #[serde(default)]
    pub len: u64,
    #[serde(default)]
    pub watched: bool,
    /// Unix time this was last watched (for "Next episode" and "Continue watching").
    #[serde(default)]
    pub at: u64,
}

/// Movies are saved under their id; episodes under "<show id>|<file name>".
pub fn key(id: &str, episode_file: Option<&str>) -> String {
    match episode_file {
        Some(f) => format!("{id}|{f}"),
        None => id.to_string(),
    }
}

/// Watched means past 90%, or within the last minute (end credits).
pub fn is_finished(pos: u64, len: u64) -> bool {
    len > 0 && (pos * 10 >= len * 9 || pos + 60 >= len)
}

/// Less than this far in isn't worth resuming.
pub const MIN_RESUME: u64 = 30;

/// What to save when the player closes, given what was saved before.
pub fn after_session(before: Option<Progress>, pos: u64, len: u64, now: u64) -> Progress {
    let was = before.unwrap_or_default();
    if is_finished(pos, len) {
        return Progress { pos: 0, len, watched: true, at: now };
    }
    // Watched it before and only peeked this time: stays watched, no resume point.
    if was.watched && pos < MIN_RESUME {
        return Progress { at: now, len: len.max(was.len), ..was };
    }
    Progress { pos: if pos >= MIN_RESUME { pos } else { 0 }, len, watched: was.watched, at: now }
}

/// Where to start: the saved spot, unless it's finished or barely started.
pub fn start_at(saved: Option<&Progress>) -> Option<u64> {
    saved.filter(|p| p.pos >= MIN_RESUME && !is_finished(p.pos, p.len)).map(|p| p.pos)
}

/// Extra VLC options: its web remote on this PC only, and where to start.
/// They go before the file name.
pub fn vlc_args(port: u16, password: &str, start: Option<u64>) -> Vec<String> {
    let mut a = vec![
        "--extraintf=http".to_string(),
        "--http-host=127.0.0.1".to_string(),
        format!("--http-port={port}"),
        format!("--http-password={password}"),
    ];
    if let Some(s) = start {
        a.push(format!("--start-time={s}"));
    }
    a
}

/// A free port on this PC for VLC's remote.
pub fn free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0").ok()?.local_addr().ok().map(|a| a.port())
}

/// A throwaway password for one viewing.
pub fn password() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    format!("{:016x}", h.finish())
}

/// (seconds in, length) from VLC's status.json. None until a video is loaded.
pub fn parse_vlc_status(json: &str) -> Option<(u64, u64)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let num = |k: &str| v.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok())));
    let len = num("length").filter(|l| *l > 0.0)?;
    // "position" (0..1) is finer than "time" (whole seconds); use whichever is further.
    let t = num("time").unwrap_or(0.0).max(num("position").unwrap_or(0.0) * len);
    Some((t.max(0.0).round() as u64, len.round() as u64))
}

/// Ask VLC how far in it is. A plain request over a local connection (no
/// proxies, no extra libraries; it only ever talks to this PC).
pub fn poll_vlc(port: u16, password: &str) -> Option<(u64, u64)> {
    use std::io::{Read, Write};
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut c = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok()?;
    c.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    let auth = base64(format!(":{password}").as_bytes());
    write!(c, "GET /requests/status.json HTTP/1.0\r\nHost: 127.0.0.1\r\nAuthorization: Basic {auth}\r\nConnection: close\r\n\r\n").ok()?;
    let mut buf = Vec::new();
    c.take(1 << 20).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let (head, body) = text.split_once("\r\n\r\n")?;
    if !head.starts_with("HTTP/1.") || head.split(' ').nth(1) != Some("200") {
        return None;
    }
    parse_vlc_status(body)
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// The script mpv runs for Spine (mpv has Lua built in). It only writes one
/// small file, named by Spine in `--script-opts=spine-progress=<file>`.
pub const MPV_SCRIPT: &str = r#"-- Spine: saves how far into the video you are, so Spine can resume it later.
local out = mp.get_opt("spine-progress")
if not out then return end
local function save()
  local pos = mp.get_property_number("time-pos")
  local len = mp.get_property_number("duration")
  if pos and len and len > 0 then
    local f = io.open(out, "w")
    if f then
      f:write(string.format("%d %d\n", math.floor(pos + 0.5), math.floor(len + 0.5)))
      f:close()
    end
  end
end
mp.add_periodic_timer(2, save)
mp.add_hook("on_unload", 50, save)
"#;

/// Extra mpv options: Spine's script, the file it writes, and where to start.
/// They go before the file name.
pub fn mpv_args(script: &std::path::Path, out: &std::path::Path, start: Option<u64>) -> Vec<String> {
    // "%<length>%value" keeps a comma or "=" in the folder name from splitting the option.
    let out = out.display().to_string();
    let mut a = vec![format!("--script={}", script.display()), format!("--script-opts=spine-progress=%{}%{out}", out.len())];
    if let Some(s) = start {
        a.push(format!("--start={s}"));
    }
    a
}

/// (seconds in, length) from the file Spine's mpv script writes.
pub fn parse_mpv_progress(text: &str) -> Option<(u64, u64)> {
    let mut it = text.split_whitespace().map(|x| x.parse::<u64>().ok());
    let (pos, len) = (it.next()??, it.next()??);
    (len > 0).then_some((pos.min(len), len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_rules() {
        assert!(is_finished(1300, 1440)); // last minute and a half of an episode
        assert!(is_finished(6500, 7200)); // 90% of a movie
        assert!(!is_finished(600, 1440));
        assert!(!is_finished(100, 0));
    }

    #[test]
    fn sessions() {
        let p = after_session(None, 754, 1440, 10);
        assert_eq!(p, Progress { pos: 754, len: 1440, watched: false, at: 10 });
        assert_eq!(start_at(Some(&p)), Some(754));
        let done = after_session(Some(p), 1420, 1440, 20);
        assert!(done.watched && done.pos == 0);
        assert_eq!(start_at(Some(&done)), None);
        // Rewatching: a quick peek keeps it watched.
        assert!(after_session(Some(done), 5, 1440, 30).watched);
        // Rewatching halfway: still watched, and resumable.
        let re = after_session(Some(done), 700, 1440, 40);
        assert!(re.watched && start_at(Some(&re)) == Some(700));
        // Barely started: nothing to resume.
        assert_eq!(start_at(Some(&after_session(None, 12, 1440, 1))), None);
    }

    #[test]
    fn vlc_status() {
        let j = r#"{"time":754,"length":1440,"position":0.5240,"state":"playing"}"#;
        assert_eq!(parse_vlc_status(j), Some((755, 1440)));
        assert_eq!(parse_vlc_status(r#"{"time":0,"length":0,"state":"stopped"}"#), None);
        assert_eq!(parse_vlc_status("not json"), None);
    }

    #[test]
    fn mpv_progress() {
        assert_eq!(parse_mpv_progress("754 1440\n"), Some((754, 1440)));
        assert_eq!(parse_mpv_progress("1500 1440"), Some((1440, 1440)));
        assert_eq!(parse_mpv_progress("12 0"), None);
        assert_eq!(parse_mpv_progress("75"), None); // caught mid-write
        assert_eq!(parse_mpv_progress(""), None);
        let a = mpv_args(std::path::Path::new("/s/spine.lua"), std::path::Path::new("/s/pos.txt"), Some(60));
        assert_eq!(a, vec!["--script=/s/spine.lua".to_string(), "--script-opts=spine-progress=%10%/s/pos.txt".into(), "--start=60".into()]);
        assert!(MPV_SCRIPT.contains("on_unload") && MPV_SCRIPT.contains("%d %d\\n"));
    }

    #[test]
    fn keys_and_args() {
        assert_eq!(key("tv/jjk", Some("JJK S01E02.mkv")), "tv/jjk|JJK S01E02.mkv");
        assert_eq!(key("movies/up", None), "movies/up");
        let a = vlc_args(9001, "pw", Some(60));
        assert!(a.contains(&"--http-port=9001".into()) && a.contains(&"--start-time=60".into()));
        assert_eq!(base64(b":abc"), "OmFiYw==");
        assert_eq!(base64(b":pw"), "OnB3");
        assert!(free_port().is_some());
    }
}
