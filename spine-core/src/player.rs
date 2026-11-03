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

/// The script mpv runs for Spine (mpv has Lua built in). It writes one small
/// file, named by Spine in `--script-opts=spine-progress=<file>`: "<seconds> <length>",
/// plus " next" once the "Up next" countdown has run out (or Enter was pressed), so
/// Spine knows to start the next episode. With `spine-next=<title>` the video stays
/// open on its last frame (Spine passes `--keep-open=yes`) and an "Up next" card counts
/// down `spine-wait` seconds. Esc (or B on a controller) cancels.
pub const MPV_SCRIPT: &str = r#"-- Spine: remembers how far in you are, and counts down to the next episode.
local out = mp.get_opt("spine-progress")
if not out then return end
local nxt = mp.get_opt("spine-next") or ""
local wait = tonumber(mp.get_opt("spine-wait") or "") or 10
local want_next = false

local function save()
  local pos = mp.get_property_number("time-pos")
  local len = mp.get_property_number("duration")
  if pos and len and len > 0 then
    local f = io.open(out, "w")
    if f then
      f:write(string.format("%d %d%s\n", math.floor(pos + 0.5), math.floor(len + 0.5), want_next and " next" or ""))
      f:close()
    end
  end
end
mp.add_periodic_timer(2, save)
mp.add_hook("on_unload", 50, save)

if nxt == "" then return end

-- "Up next" card, bottom right, drawn on a 1920x1080 canvas (mpv scales it).
local card = mp.create_osd_overlay("ass-events")
card.res_x, card.res_y = 1920, 1080
local timer, left = nil, 0
local keys = { "ENTER", "KP_ENTER", "GAMEPAD_ACTION_DOWN", "ESC", "GAMEPAD_ACTION_RIGHT", "BS" }

local function esc(s)
  return (s:gsub("\\", "\\\\"):gsub("{", "\\{"):gsub("}", "\\}"):gsub("\n", " "))
end
local function rect(x, y, w, h, color, alpha)
  return string.format("{\\an7\\pos(0,0)\\bord0\\shad0\\1c&H%s&\\1a&H%s&\\p1}m %d %d l %d %d l %d %d l %d %d{\\p0}",
    color, alpha, x, y, x + w, y, x + w, y + h, x, y + h)
end
local function draw()
  local x, y, w, h = 1220, 790, 640, 220
  local done = (wait - left) / wait
  local ev = {
    rect(x, y, w, h, "141414", "28"),
    rect(x, y + h - 8, w, 8, "FFFFFF", "C0"),
    rect(x, y + h - 8, math.floor(w * done + 0.5), 8, "FFFFFF", "00"),
    string.format("{\\an7\\pos(%d,%d)\\fs26\\bord0\\shad0\\1c&HB4B4B4&\\b1}UP NEXT", x + 36, y + 30),
    string.format("{\\an7\\pos(%d,%d)\\fs44\\bord0\\shad0\\1c&HFFFFFF&\\b1\\q2}%s", x + 36, y + 66, esc(nxt)),
    string.format("{\\an7\\pos(%d,%d)\\fs28\\bord0\\shad0\\1c&HDCDCDC&}Plays in %d   ·   Enter: play now   ·   Esc: stay here", x + 36, y + 142, left),
  }
  card.data = table.concat(ev, "\n")
  card:update()
end
local function stop()
  if timer then timer:kill() timer = nil end
  card:remove()
  for _, k in ipairs(keys) do mp.remove_key_binding("spine-" .. k) end
end
local function go()
  stop()
  want_next = true
  save()
  mp.command("quit")
end
local function start()
  if timer then return end
  left = wait
  draw()
  for i, k in ipairs(keys) do
    mp.add_forced_key_binding(k, "spine-" .. k, i <= 3 and go or stop)
  end
  timer = mp.add_periodic_timer(1, function()
    left = left - 1
    if left <= 0 then go() else draw() end
  end)
end
mp.observe_property("eof-reached", "bool", function(_, v)
  if v then start() else stop() end
end)
"#;

/// The "Up next" countdown in mpv: the next episode's name and how many seconds to wait.
#[derive(Debug, Clone, PartialEq)]
pub struct UpNext {
    pub title: String,
    pub wait: u64,
}

/// Extra mpv options: Spine's script, the file it writes, where to start, and (for
/// "Up next") the next episode's name. They go before the file name.
pub fn mpv_args(script: &std::path::Path, out: &std::path::Path, start: Option<u64>, next: Option<&UpNext>) -> Vec<String> {
    // "%<length>%value" keeps a comma or "=" in a name from splitting the option.
    let opt = |k: &str, v: &str| format!("{k}=%{}%{v}", v.len());
    let out = out.display().to_string();
    let mut opts = vec![opt("spine-progress", &out)];
    if let Some(n) = next {
        opts.push(opt("spine-next", &n.title));
        opts.push(format!("spine-wait={}", n.wait.max(1)));
    }
    let mut a = vec![format!("--script={}", script.display()), format!("--script-opts={}", opts.join(","))];
    if next.is_some() {
        // Stay on the last frame so the countdown can show.
        a.push("--keep-open=yes".into());
    }
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

/// Did the "Up next" countdown finish (so Spine should play the next episode)?
pub fn mpv_wants_next(text: &str) -> bool {
    text.split_whitespace().nth(2) == Some("next")
}

/// Language codes the players look for (files tag tracks with 2- or 3-letter codes).
fn lang_codes(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "en" => "eng,en,en-US,en-GB",
        "ja" => "jpn,ja,jp",
        "es" => "spa,es,es-419,es-ES",
        "fr" => "fre,fra,fr",
        "de" => "ger,deu,de",
        "pt" => "por,pt,pt-BR",
        "it" => "ita,it",
        "ko" => "kor,ko",
        _ => return None,
    })
}

/// Preferred audio and subtitle languages (Settings: audioLang / subLang).
/// audio: "" = the file's own choice, or a language ("ja"). subs: "" = the file's
/// own choice, "off" (mpv only), or a language. `player` is "mpv" or "VLC".
pub fn lang_args(player: &str, audio: &str, subs: &str) -> Vec<String> {
    let mut a = Vec::new();
    let vlc = player == "VLC";
    if let Some(c) = lang_codes(audio) {
        a.push(if vlc { format!("--audio-language={c}") } else { format!("--alang={c}") });
    }
    if subs == "off" {
        if !vlc {
            a.push("--sid=no".into());
        }
    } else if let Some(c) = lang_codes(subs) {
        a.push(if vlc { format!("--sub-language={c}") } else { format!("--slang={c}") });
        if !vlc {
            // Show them even when the file marks no subtitle track as default.
            a.push("--sid=auto".into());
        }
    }
    a
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
        let a = mpv_args(std::path::Path::new("/s/spine.lua"), std::path::Path::new("/s/pos.txt"), Some(60), None);
        assert_eq!(a, vec!["--script=/s/spine.lua".to_string(), "--script-opts=spine-progress=%10%/s/pos.txt".into(), "--start=60".into()]);
        assert!(MPV_SCRIPT.contains("on_unload") && MPV_SCRIPT.contains("eof-reached"));
        // Up next: the title is length-prefixed so its commas survive, and the video stays open.
        let t = "S1 · E4 Cursed, Womb";
        let n = UpNext { title: t.into(), wait: 10 };
        let a = mpv_args(std::path::Path::new("/s/spine.lua"), std::path::Path::new("/s/pos.txt"), None, Some(&n));
        assert_eq!(a[1], format!("--script-opts=spine-progress=%10%/s/pos.txt,spine-next=%{}%{t},spine-wait=10", t.len()));
        assert_eq!(a[2], "--keep-open=yes");
        assert!(mpv_wants_next("1440 1440 next\n") && !mpv_wants_next("1440 1440\n") && !mpv_wants_next(""));
        assert_eq!(parse_mpv_progress("1440 1440 next"), Some((1440, 1440)));
    }

    #[test]
    fn languages() {
        assert_eq!(lang_args("mpv", "ja", "en"), vec!["--alang=jpn,ja,jp", "--slang=eng,en,en-US,en-GB", "--sid=auto"]);
        assert_eq!(lang_args("mpv", "", "off"), vec!["--sid=no"]);
        assert_eq!(lang_args("VLC", "ja", "en"), vec!["--audio-language=jpn,ja,jp", "--sub-language=eng,en,en-US,en-GB"]);
        assert!(lang_args("mpv", "", "").is_empty() && lang_args("VLC", "xx", "zz").is_empty());
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
