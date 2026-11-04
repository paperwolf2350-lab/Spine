-- Spine's player look for mpv: its own controls, Skip intro / Skip credits,
-- an audio & subtitle menu, the "Up next" countdown, and saving where you are.
-- Spine passes everything in --script-opts (spine-progress, spine-title, spine-sub,
-- spine-accent, spine-skip, spine-next, spine-wait) and starts mpv with --osc=no.

local out = mp.get_opt("spine-progress")
if not out then return end
local function opt(k, d) local v = mp.get_opt(k); if v == nil or v == "" then return d end; return v end
local TITLE = opt("spine-title", "")
local SUB = opt("spine-sub", "")
local NEXT = opt("spine-next", "")
local WAIT = math.max(1, tonumber(opt("spine-wait", "10")) or 10)
local SKIP = opt("spine-skip", "button") -- button | auto | off
local accent = opt("spine-accent", "C8431F"):gsub("#", "")
if not accent:match("^%x%x%x%x%x%x$") then accent = "C8431F" end
local ACC = accent:sub(5, 6) .. accent:sub(3, 4) .. accent:sub(1, 2) -- ASS colours are BBGGRR

-- ---------------------------------------------------------------- progress
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

-- ---------------------------------------------------------------- drawing helpers
local W, H, S = 1920, 1080, 1
local ov = mp.create_osd_overlay("ass-events")
local ev, hits = {}, {}
local mouse = { x = -1, y = -1, inside = false }

local function esc(s) return (tostring(s):gsub("\\", "\\\\"):gsub("{", "\\{"):gsub("}", "\\}"):gsub("\n", " ")) end
local function n(v) return math.floor(v + 0.5) end
local function shape(color, alpha, draw, extra)
  ev[#ev + 1] = string.format("{\\an7\\pos(0,0)\\bord0\\shad0\\1c&H%s&\\1a&H%s&%s\\p1}%s{\\p0}", color, alpha, extra or "", draw)
end
local function outline(color, alpha, width, draw)
  ev[#ev + 1] = string.format("{\\an7\\pos(0,0)\\bord%s\\shad0\\1a&HFF&\\3c&H%s&\\3a&H%s&\\p1}%s{\\p0}", width, color, alpha, draw)
end
local function text(x, y, an, size, color, s, bold, alpha)
  ev[#ev + 1] = string.format("{\\an%d\\pos(%d,%d)\\fs%d\\bord0\\shad0\\1c&H%s&\\1a&H%s&\\b%d\\q2}%s",
    an, n(x), n(y), n(size), color, alpha or "00", bold and 1 or 0, esc(s))
end
local function rect(x, y, w, h)
  return string.format("m %d %d l %d %d l %d %d l %d %d ", n(x), n(y), n(x + w), n(y), n(x + w), n(y + h), n(x), n(y + h))
end
local function rrect(x, y, w, h, r)
  r = math.min(r, w / 2, h / 2)
  local k = r * 0.4477
  return string.format("m %d %d l %d %d b %d %d %d %d %d %d l %d %d b %d %d %d %d %d %d l %d %d b %d %d %d %d %d %d l %d %d b %d %d %d %d %d %d ",
    n(x + r), n(y), n(x + w - r), n(y), n(x + w - k), n(y), n(x + w), n(y + k), n(x + w), n(y + r),
    n(x + w), n(y + h - r), n(x + w), n(y + h - k), n(x + w - k), n(y + h), n(x + w - r), n(y + h),
    n(x + r), n(y + h), n(x + k), n(y + h), n(x), n(y + h - k), n(x), n(y + h - r),
    n(x), n(y + r), n(x), n(y + k), n(x + k), n(y), n(x + r), n(y))
end
local function circle(cx, cy, r) return rrect(cx - r, cy - r, 2 * r, 2 * r, r) end
local function poly(pts)
  local s = string.format("m %d %d ", n(pts[1]), n(pts[2]))
  for i = 3, #pts, 2 do s = s .. string.format("l %d %d ", n(pts[i]), n(pts[i + 1])) end
  return s
end
local function hit(x, y, w, h, act, key)
  hits[#hits + 1] = { x = x, y = y, w = w, h = h, act = act, key = key }
  return mouse.inside and mouse.x >= x and mouse.x <= x + w and mouse.y >= y and mouse.y <= y + h
end
local function fmt(t)
  t = math.max(0, math.floor(t or 0))
  local h, m, s = math.floor(t / 3600), math.floor(t / 60) % 60, t % 60
  if h > 0 then return string.format("%d:%02d:%02d", h, m, s) end
  return string.format("%d:%02d", m, s)
end
local function gradient(top, y, h)
  -- Bands that meet exactly (an overlap would show as dark lines); eased so it fades softly.
  local steps = 36
  for i = 0, steps - 1 do
    local a = top and i / steps or (steps - 1 - i) / steps
    local alpha = string.format("%02X", n(0x38 + (0xFF - 0x38) * (a ^ 0.8)))
    local y0, y1 = n(y + h * i / steps), n(y + h * (i + 1) / steps)
    if y1 > y0 then
      ev[#ev + 1] = string.format("{\\an7\\pos(0,0)\\bord0\\shad0\\1c&H000000&\\1a&H%s&\\p1}m 0 %d l %d %d l %d %d l 0 %d{\\p0}", alpha, y0, n(W), y0, n(W), y1, y1)
    end
  end
end

-- ---------------------------------------------------------------- state
local visible_until = 0
local menu = false
local drag = false
local upnext = nil -- { left = seconds }
local skipped = {} -- chapters already auto-skipped
local pad = false -- the controller was used last (shows button hints and the menu focus)
local msel = { c = 1, r = 1 } -- focused row in the audio & subtitles menu (controller)
local function now() return mp.get_time() end
local function wake(secs) visible_until = math.max(visible_until, now() + (secs or 2.5)) end

local function chapters() return mp.get_property_native("chapter-list") or {} end
local function kind_of(title)
  local t = (title or ""):lower():gsub("^%s+", "")
  if t == "op" or t:match("^op%s*%d") or t:find("opening", 1, true) or t:match("^intro") or t:find("theme song", 1, true) or t:find("title sequence", 1, true) then return "intro" end
  if t == "ed" or t:match("^ed%s*%d") or t:find("ending", 1, true) or t:find("credits", 1, true) or t:match("^outro") or t:find("preview", 1, true) then return "credits" end
  return nil
end
-- Intro / credits / recap times from AniSkip (Spine writes them to a file once it has them),
-- used when the file has no Opening / Ending chapters of its own.
local SKIPS_FILE = opt("spine-skips", "")
local web = nil
if SKIPS_FILE ~= "" then
  local tries, t = 0, nil
  t = mp.add_periodic_timer(2, function()
    tries = tries + 1
    local f = io.open(SKIPS_FILE, "r")
    if f then
      local list = {}
      for line in f:lines() do
        local k, a, b = line:match("^(%a+)%s+([%d%.]+)%s+([%d%.]+)")
        if k then
          list[#list + 1] = { kind = (k == "op" and "intro") or (k == "ed" and "credits") or "recap", from = tonumber(a), to = tonumber(b) }
        end
      end
      f:close()
      web = list
      t:kill()
    elseif tries > 45 then
      t:kill()
    end
  end)
end
local function has_chapter_parts()
  for _, c in ipairs(chapters()) do if kind_of(c.title) then return true end end
  return false
end

-- The intro/credits part we're in: { kind, from, to, index } from a chapter, or
-- { kind, from, to, web = true, tail } from AniSkip.
local function current_part()
  local i = mp.get_property_number("chapter")
  local list = chapters()
  if i and i >= 0 and list[i + 1] then
    local k = kind_of(list[i + 1].title)
    if k then
      local to = list[i + 2] and list[i + 2].time or mp.get_property_number("duration")
      return { kind = k, from = list[i + 1].time, to = to, index = i, last = list[i + 2] == nil }
    end
  end
  if web and #web > 0 and not has_chapter_parts() then
    local pos = mp.get_property_number("time-pos") or 0
    local len = mp.get_property_number("duration") or 0
    for _, p in ipairs(web) do
      if pos >= p.from and pos < p.to - 0.5 then
        return { kind = p.kind, from = p.from, to = p.to, web = true, tail = len > 0 and (len - p.to) or 0 }
      end
    end
  end
  return nil
end

local quit_next, start_upnext, stop_upnext

local function do_skip(part)
  if not part then return end
  if part.web then
    -- AniSkip credits close to the end (a short preview at most): on to the next episode.
    if part.kind == "credits" and NEXT ~= "" and part.tail < 45 then return quit_next() end
    return mp.commandv("seek", tostring(part.to), "absolute+exact")
  end
  -- Credits with nothing after them but more credits / a preview: on to the next episode.
  local list, rest_is_credits = chapters(), true
  for j = part.index + 2, #list do if kind_of(list[j].title) ~= "credits" then rest_is_credits = false end end
  if part.kind == "credits" and NEXT ~= "" and rest_is_credits then return quit_next() end
  if part.to then mp.commandv("seek", tostring(part.to), "absolute+exact") end
end

-- ---------------------------------------------------------------- icons
local function ic_play(cx, cy, r) shape("FFFFFF", "00", poly({ cx - r * 0.42, cy - r * 0.55, cx + r * 0.58, cy, cx - r * 0.42, cy + r * 0.55 })) end
local function ic_pause(cx, cy, r) shape("FFFFFF", "00", rect(cx - r * 0.45, cy - r * 0.55, r * 0.3, r * 1.1) .. rect(cx + r * 0.15, cy - r * 0.55, r * 0.3, r * 1.1)) end
local function ic_jump(cx, cy, r, fwd)
  outline("FFFFFF", "00", n(2.6 * S), circle(cx, cy, r * 0.62))
  local d = fwd and 1 or -1
  shape("FFFFFF", "00", poly({ cx + d * r * 0.05, cy - r * 0.86, cx + d * r * 0.42, cy - r * 0.62, cx + d * r * 0.05, cy - r * 0.38 }))
  text(cx, cy + 1, 5, r * 0.62, "FFFFFF", "10", true)
end
local function ic_volume(cx, cy, r, muted)
  shape("FFFFFF", "00", poly({ cx - r * 0.7, cy - r * 0.22, cx - r * 0.35, cy - r * 0.22, cx + r * 0.05, cy - r * 0.6, cx + r * 0.05, cy + r * 0.6, cx - r * 0.35, cy + r * 0.22, cx - r * 0.7, cy + r * 0.22 }))
  if muted then
    text(cx + r * 0.5, cy, 5, r * 0.9, "FFFFFF", "×", true)
  else
    shape("FFFFFF", "00", rect(cx + r * 0.25, cy - r * 0.2, r * 0.1, r * 0.4) .. rect(cx + r * 0.48, cy - r * 0.38, r * 0.1, r * 0.76))
  end
end
local function ic_next(cx, cy, r)
  shape("FFFFFF", "00", poly({ cx - r * 0.5, cy - r * 0.5, cx + r * 0.3, cy, cx - r * 0.5, cy + r * 0.5 }) .. rect(cx + r * 0.36, cy - r * 0.5, r * 0.16, r))
end
local function ic_subs(cx, cy, r)
  outline("FFFFFF", "00", n(2.4 * S), rrect(cx - r * 0.7, cy - r * 0.5, r * 1.4, r, r * 0.18))
  shape("FFFFFF", "00", rect(cx - r * 0.42, cy - r * 0.05, r * 0.36, r * 0.12) .. rect(cx + r * 0.0, cy - r * 0.05, r * 0.42, r * 0.12) .. rect(cx - r * 0.42, cy + r * 0.18, r * 0.84, r * 0.12))
end
local function ic_full(cx, cy, r, on)
  local a, t = r * 0.6, r * 0.11
  local d = on and -1 or 1
  local s = ""
  for _, c in ipairs({ { -1, -1 }, { 1, -1 }, { -1, 1 }, { 1, 1 } }) do
    local x, y = cx + c[1] * a, cy + c[2] * a
    s = s .. rect(math.min(x, x - c[1] * d * r * 0.4), y - t / 2, r * 0.4, t) .. rect(x - t / 2, math.min(y, y - c[2] * d * r * 0.4), t, r * 0.4)
  end
  shape("FFFFFF", "00", s)
end
local function ic_back(cx, cy, r)
  shape("FFFFFF", "00", poly({ cx + r * 0.2, cy - r * 0.62, cx + r * 0.32, cy - r * 0.5, cx - r * 0.12, cy, cx + r * 0.32, cy + r * 0.5, cx + r * 0.2, cy + r * 0.62, cx - r * 0.38, cy }))
end

-- A round button with a soft hover circle.
local function button(cx, cy, r, key, act, draw)
  local on = hit(cx - r, cy - r, 2 * r, 2 * r, act, key)
  if on then shape("FFFFFF", "D8", circle(cx, cy, r)) end
  draw(cx, cy, r * 0.82)
end
-- A pill with a label (Skip intro, Play now...). filled = white with dark text.
local function pill(x, y, label, act, key, filled)
  local fs = 30 * S
  local w, h = (#label * 0.56 * fs) + 64 * S, 68 * S
  local on = hit(x - w, y - h, w, h, act, key)
  if filled or on then
    shape("FFFFFF", "00", rrect(x - w, y - h, w, h, 10 * S))
    text(x - w / 2, y - h / 2, 5, fs, "141414", label, true)
  else
    shape("141414", "50", rrect(x - w, y - h, w, h, 10 * S))
    outline("FFFFFF", "40", n(2 * S), rrect(x - w, y - h, w, h, 10 * S))
    text(x - w / 2, y - h / 2, 5, fs, "FFFFFF", label, true)
  end
  return w
end

-- ---------------------------------------------------------------- tracks menu
local function track_label(t)
  local parts = {}
  if t.lang then parts[#parts + 1] = t.lang:upper() end
  if t.title and t.title ~= "" then parts[#parts + 1] = t.title end
  if #parts == 0 then parts[1] = (t.type == "audio" and "Track " or "Subtitles ") .. t.id end
  local s = table.concat(parts, " · ")
  if #s > 34 then s = s:sub(1, 33) .. "…" end
  return s
end
-- The menu's two columns: { prop, title, rows = { {id, label, selected} } }.
local function menu_cols()
  local tracks = mp.get_property_native("track-list") or {}
  local out = {}
  for _, c in ipairs({ { "audio", "Audio", "aid" }, { "sub", "Subtitles", "sid" } }) do
    local rows = {}
    if c[1] == "sub" then rows[1] = { id = "no", label = "Off", selected = mp.get_property("sid") == "no" } end
    for _, t in ipairs(tracks) do
      if t.type == c[1] then rows[#rows + 1] = { id = t.id, label = track_label(t), selected = t.selected } end
    end
    out[#out + 1] = { prop = c[3], title = c[2], rows = rows }
  end
  return out
end
-- Open the menu with the focus on the audio track that's playing.
local function open_menu()
  menu = true
  msel = { c = 1, r = 1 }
  for i, t in ipairs(menu_cols()[1].rows) do if t.selected then msel.r = i end end
end
local function draw_menu()
  local tracks = mp.get_property_native("track-list") or {}
  local cols = { { "audio", "Audio" }, { "sub", "Subtitles" } }
  local colw, rowh = 420 * S, 56 * S
  local rows = 1
  for _, c in ipairs(cols) do
    local k = (c[1] == "sub") and 1 or 0
    for _, t in ipairs(tracks) do if t.type == c[1] then k = k + 1 end end
    rows = math.max(rows, k)
  end
  local mw, mh = colw * 2 + 72 * S, rows * rowh + 120 * S
  local x, y = W - mw - 60 * S, H - 190 * S - mh
  shape("141414", "18", rrect(x, y, mw, mh, 18 * S))
  for ci, c in ipairs(cols) do
    local cx = x + 36 * S + (ci - 1) * colw
    text(cx, y + 34 * S, 7, 26 * S, "B4B4B4", c[2], true)
    local list = {}
    if c[1] == "sub" then list[1] = { id = "no", label = "Off", selected = mp.get_property("sid") == "no" } end
    for _, t in ipairs(tracks) do
      if t.type == c[1] then list[#list + 1] = { id = t.id, label = track_label(t), selected = t.selected } end
    end
    for i, t in ipairs(list) do
      local ry = y + 84 * S + (i - 1) * rowh
      local prop = c[1] == "audio" and "aid" or "sid"
      local on = hit(cx - 12 * S, ry, colw - 16 * S, rowh - 6 * S, function() mp.set_property(prop, tostring(t.id)) end, prop .. t.id)
      if on then shape("FFFFFF", "D8", rrect(cx - 12 * S, ry, colw - 16 * S, rowh - 6 * S, 8 * S)) end
      if pad and msel.c == ci and msel.r == i then
        shape("FFFFFF", "D0", rrect(cx - 12 * S, ry, colw - 16 * S, rowh - 6 * S, 8 * S))
        outline(ACC, "00", n(3 * S), rrect(cx - 12 * S, ry, colw - 16 * S, rowh - 6 * S, 8 * S))
      end
      if t.selected then shape(ACC, "00", circle(cx + 8 * S, ry + rowh / 2 - 3 * S, 7 * S)) end
      text(cx + 30 * S, ry + rowh / 2 - 3 * S, 4, 28 * S, t.selected and "FFFFFF" or "DCDCDC", t.label, t.selected)
    end
  end
end

-- ---------------------------------------------------------------- the whole UI
local function render()
  ev, hits = {}, {}
  local paused = mp.get_property_bool("pause")
  local pos = mp.get_property_number("time-pos") or 0
  local len = mp.get_property_number("duration") or 0
  local show = paused or menu or drag or upnext ~= nil or now() < visible_until
  local part = (SKIP ~= "off") and current_part() or nil

  if show then
    gradient(true, 0, 230 * S)
    gradient(false, H - 300 * S, 300 * S)
    button(72 * S, 84 * S, 34 * S, "back", function() mp.command("quit") end, ic_back)
    if TITLE ~= "" then text(124 * S, 62 * S, 4, 54 * S, "FFFFFF", TITLE, true) end
    if SUB ~= "" then text(124 * S, 116 * S, 4, 32 * S, "D2D2D2", SUB, false) end

    -- Seek bar: chapters as small gaps, accent fill, a knob that grows on hover.
    local bx, bw, by = 60 * S, W - 260 * S, H - 140 * S
    local over = hit(bx, by - 22 * S, bw, 44 * S, "seek", "seek")
    local th = (over or drag) and 10 * S or 6 * S
    shape("FFFFFF", "A0", rrect(bx, by - th / 2, bw, th, th / 2))
    if len > 0 then
      local fw = bw * math.min(1, pos / len)
      shape(ACC, "00", rrect(bx, by - th / 2, math.max(th, fw), th, th / 2))
      local gaps = ""
      for _, c in ipairs(chapters()) do
        if c.time > 1 and c.time < len - 1 then gaps = gaps .. rect(bx + bw * c.time / len - 1.5 * S, by - th, 3 * S, th * 2) end
      end
      if gaps ~= "" then shape("000000", "40", gaps) end
      shape("FFFFFF", "00", circle(bx + fw, by, (over or drag) and 14 * S or 9 * S))
      if over or drag then
        local mt = math.max(0, math.min(1, (mouse.x - bx) / bw)) * len
        local tw = 110 * S
        local tx = math.max(bx, math.min(bx + bw - tw, mouse.x - tw / 2))
        shape("141414", "20", rrect(tx, by - 74 * S, tw, 44 * S, 8 * S))
        text(tx + tw / 2, by - 52 * S, 5, 26 * S, "FFFFFF", fmt(mt), true)
      end
      text(W - 60 * S, by, 6, 28 * S, "FFFFFF", "-" .. fmt(len - pos), false)
    end

    if pad then
      -- Controller hints, above the seek bar on the left (the menu and Skip sit on the right).
      local fs, gr = 24 * S, 17 * S
      local list = upnext and { { "A", "Play now" }, { "B", "Cancel" } }
        or menu and { { "A", "Choose" }, { "B", "Close menu" } }
        or { { "A", paused and "Play" or "Pause" }, { "B", "Close video" }, { "Y", "Audio & subtitles" } }
      if not menu and not upnext and NEXT ~= "" then list[#list + 1] = { "X", "Next episode" } end
      if not menu and not upnext then list[#list + 1] = { "\226\151\128\226\150\182", "10 s" }; list[#list + 1] = { "LB RB", "1 min" } end
      local widths, total = {}, 0
      for i, h in ipairs(list) do
        local gw = (#h[1] > 1 and (#h[1] > 3 and 2.6 or 1.6) or 1) * 2 * gr
        widths[i] = gw + 10 * S + #h[2] * 0.44 * fs + 30 * S
        total = total + widths[i]
      end
      local hx, hy = bx, by - 58 * S
      local tint = { A = "4FA34A", B = "3C3CD8", X = "D8783C", Y = "2CB8E0" } -- BBGGRR
      for i, h in ipairs(list) do
        local gw = (#h[1] > 1 and (#h[1] > 3 and 2.6 or 1.6) or 1) * 2 * gr
        shape(tint[h[1]] or "3C3C3C", "10", rrect(hx, hy - gr, gw, 2 * gr, gr))
        text(hx + gw / 2, hy, 5, (#h[1] > 3 and 0.62 or 0.8) * fs, "FFFFFF", h[1], true)
        text(hx + gw + 10 * S, hy, 4, fs, "E6E6E6", h[2], false)
        hx = hx + widths[i]
      end
    end

    -- Controls row.
    local cy, r = H - 66 * S, 32 * S
    button(84 * S, cy, r, "play", function() mp.command("cycle pause") end, paused and ic_play or ic_pause)
    button(166 * S, cy, r, "rew", function() mp.commandv("seek", "-10") end, function(x, y, rr) ic_jump(x, y, rr, false) end)
    button(248 * S, cy, r, "fwd", function() mp.commandv("seek", "10") end, function(x, y, rr) ic_jump(x, y, rr, true) end)
    local muted = mp.get_property_bool("mute")
    button(330 * S, cy, r, "vol", function() mp.command("cycle mute") end, function(x, y, rr) ic_volume(x, y, rr, muted) end)
    text(386 * S, cy, 4, 28 * S, "DCDCDC", fmt(pos) .. "  /  " .. fmt(len), false)
    local fx = W - 84 * S
    button(fx, cy, r, "full", function() mp.command("cycle fullscreen") end, function(x, y, rr) ic_full(x, y, rr, mp.get_property_bool("fullscreen")) end)
    button(fx - 82 * S, cy, r, "subs", function() if menu then menu = false else open_menu() end end, ic_subs)
    if NEXT ~= "" then button(fx - 164 * S, cy, r, "next", function() quit_next() end, ic_next) end
    if menu then draw_menu() end
  end

  -- Skip intro / credits: shown the whole time you're in that part, controls or not.
  if part and not upnext and not menu then
    local next_ep = NEXT ~= "" and (not part.web or part.tail < 45)
    local label = (part.kind == "intro" and "Skip intro") or (part.kind == "recap" and "Skip recap") or (next_ep and "Next episode" or "Skip credits")
    pill(W - 60 * S, H - (show and 200 or 80) * S, label, function() do_skip(part) end, "skip", false)
  end

  -- Up next card.
  if upnext then
    local cw, ch = 640 * S, 250 * S
    local x, y = W - 60 * S - cw, H - (show and 200 or 80) * S - ch
    shape("141414", "18", rrect(x, y, cw, ch, 18 * S))
    text(x + 36 * S, y + 40 * S, 4, 26 * S, ACC, "UP NEXT", true)
    text(x + 36 * S, y + 86 * S, 4, 40 * S, "FFFFFF", NEXT, true)
    text(x + 36 * S, y + 134 * S, 4, 28 * S, "C8C8C8", string.format("Plays in %d", upnext.left), false)
    local bw2 = pill(x + cw - 36 * S, y + ch - 28 * S, "Play now", function() quit_next() end, "now", true)
    pill(x + cw - 36 * S - bw2 - 16 * S, y + ch - 28 * S, "Cancel", function() stop_upnext() end, "cancel", false)
    local done = (WAIT - upnext.left) / WAIT
    shape(ACC, "00", rect(x + 18 * S, y + ch - 6 * S, (cw - 36 * S) * done, 4 * S))
  end

  ov.res_x, ov.res_y = W, H
  ov.data = table.concat(ev, "\n")
  ov:update()
end

-- ---------------------------------------------------------------- Up next
local up_timer = nil
function stop_upnext()
  if up_timer then up_timer:kill() up_timer = nil end
  upnext = nil
  render()
end
function quit_next()
  if NEXT == "" then return end
  want_next = true
  save()
  mp.command("quit")
end
function start_upnext()
  if upnext or NEXT == "" then return end
  upnext = { left = WAIT }
  up_timer = mp.add_periodic_timer(1, function()
    upnext.left = upnext.left - 1
    if upnext.left <= 0 then quit_next() else render() end
  end)
  render()
end
mp.observe_property("eof-reached", "bool", function(_, v)
  if v then start_upnext() elseif upnext then stop_upnext() end
end)

-- ---------------------------------------------------------------- input
local function seek_to(x)
  local len = mp.get_property_number("duration") or 0
  local bx, bw = 60 * S, W - 260 * S
  if len > 0 then mp.commandv("seek", tostring(math.max(0, math.min(1, (x - bx) / bw)) * len), "absolute") end
end
local function find_hit()
  for i = #hits, 1, -1 do
    local h = hits[i]
    if mouse.x >= h.x and mouse.x <= h.x + h.w and mouse.y >= h.y and mouse.y <= h.y + h.h then return h end
  end
end
mp.add_forced_key_binding("MBTN_LEFT", "spine-click", function(e)
  if e.event == "down" then
    -- Use where the pointer is right now, and lay out the controls before looking for a button.
    local m = mp.get_property_native("mouse-pos")
    if m then mouse.x, mouse.y, mouse.inside = m.x, m.y, true end
    local was_shown = mp.get_property_bool("pause") or menu or upnext ~= nil or now() < visible_until
    wake()
    render()
    local h = find_hit()
    -- With the controls hidden, a click only brings them back, unless it lands on Skip.
    if h and not was_shown and h.key ~= "skip" then h = nil; render(); return end
    if h and h.act == "seek" then
      drag = true
      seek_to(mouse.x)
    elseif h then
      h.act()
    elseif menu then
      menu = false
    else
      mp.command("cycle pause")
    end
    render()
  elseif e.event == "up" then
    if drag then drag = false; render() end
  end
end, { complex = true })
mp.add_forced_key_binding("MBTN_LEFT_DBL", "spine-dbl", function()
  if not find_hit() then mp.command("cycle fullscreen") end
end)

mp.observe_property("mouse-pos", "native", function(_, m)
  if not m then return end
  local moved = m.x ~= mouse.x or m.y ~= mouse.y
  mouse.x, mouse.y, mouse.inside = m.x, m.y, m.hover ~= false
  if drag then seek_to(mouse.x) end
  if moved then wake(); pad = false end
  render()
end)
mp.observe_property("osd-dimensions", "native", function(_, d)
  if d and d.w and d.w > 0 then W, H = d.w, d.h; S = math.min(W / 1920, H / 1080) end
  render()
end)

-- Enter / A: the focused menu row, the skip or Up next button if one is showing, else play/pause.
local function confirm()
  if menu then
    local col = menu_cols()[msel.c]
    local t = col and col.rows[msel.r]
    if t then mp.set_property(col.prop, tostring(t.id)) end
    return render()
  end
  if upnext then return quit_next() end
  local part = (SKIP ~= "off") and current_part() or nil
  if part then return do_skip(part) end
  mp.command("cycle pause")
  wake()
end
-- Esc: close the menu, cancel Up next, else leave fullscreen.
local function back()
  if menu then menu = false; return render() end
  if upnext then return stop_upnext() end
  if mp.get_property_bool("fullscreen") then mp.set_property_bool("fullscreen", false) end
end
local function controls_up() return mp.get_property_bool("pause") or menu or upnext ~= nil or now() < visible_until end
-- Move the menu focus (controller).
local function menu_move(dc, dr)
  local cols = menu_cols()
  msel.c = math.max(1, math.min(#cols, msel.c + dc))
  local rows = #cols[msel.c].rows
  if dc ~= 0 then
    -- Switching column: land on that column's chosen track.
    msel.r = 1
    for i, t in ipairs(cols[msel.c].rows) do if t.selected then msel.r = i end end
  end
  msel.r = math.max(1, math.min(math.max(rows, 1), msel.r + dr))
end
-- Jump d seconds, but never past the last 2 s: the end has to play for Up next to show.
local function jump(d)
  local pos = mp.get_property_number("time-pos") or 0
  local len = mp.get_property_number("duration") or 0
  local to = pos + d
  if len > 0 then to = math.min(to, math.max(0, len - 2)) end
  mp.commandv("seek", tostring(math.max(0, to)), "absolute")
end
local function volume(d)
  mp.commandv("add", "volume", tostring(d))
  mp.osd_message(string.format("Volume %d%%", n(mp.get_property_number("volume") or 0)), 1)
end

-- Controller (mpv reads it through SDL; Spine switches it on with --include, see MPV_CONF).
-- A choose · B back / show controls / close · X next episode · Y audio & subtitles ·
-- Start play/pause · D-pad / left stick: seek 10 s (left/right), volume (up/down) or move in the menu ·
-- LB / RB: 1 minute. Hold Select + Start: Spine closes the player (works without SDL too).
local function bind_pad(keys, name, fn, rep)
  for _, k in ipairs(keys) do
    mp.add_forced_key_binding(k, name .. "-" .. k, function()
      local first = not pad
      pad = true
      fn(first)
      wake()
      render()
    end, { repeatable = rep or false })
  end
end
bind_pad({ "GAMEPAD_ACTION_DOWN" }, "spine-pad-a", function() confirm() end)
bind_pad({ "GAMEPAD_ACTION_RIGHT" }, "spine-pad-b", function()
  if menu then menu = false; return end
  if upnext then return stop_upnext() end
  -- First press only shows the controls (with "B Close video"); the next one closes.
  if not controls_up() then return end
  mp.command("quit")
end)
bind_pad({ "GAMEPAD_ACTION_LEFT" }, "spine-pad-x", function() if not menu and NEXT ~= "" then quit_next() end end)
bind_pad({ "GAMEPAD_ACTION_UP" }, "spine-pad-y", function() if menu then menu = false else open_menu() end end)
bind_pad({ "GAMEPAD_START" }, "spine-pad-start", function() if not menu then mp.command("cycle pause") end end)
bind_pad({ "GAMEPAD_DPAD_LEFT", "GAMEPAD_LEFT_STICK_LEFT" }, "spine-pad-l", function() if menu then menu_move(-1, 0) else jump(-10) end end, true)
bind_pad({ "GAMEPAD_DPAD_RIGHT", "GAMEPAD_LEFT_STICK_RIGHT" }, "spine-pad-r", function() if menu then menu_move(1, 0) else jump(10) end end, true)
bind_pad({ "GAMEPAD_DPAD_UP", "GAMEPAD_LEFT_STICK_UP" }, "spine-pad-u", function() if menu then menu_move(0, -1) else volume(5) end end, true)
bind_pad({ "GAMEPAD_DPAD_DOWN", "GAMEPAD_LEFT_STICK_DOWN" }, "spine-pad-d", function() if menu then menu_move(0, 1) else volume(-5) end end, true)
bind_pad({ "GAMEPAD_LEFT_SHOULDER" }, "spine-pad-lb", function() if not menu then jump(-60) end end, true)
bind_pad({ "GAMEPAD_RIGHT_SHOULDER" }, "spine-pad-rb", function() if not menu then jump(60) end end, true)
-- Keyboard.
for _, k in ipairs({ "ENTER", "KP_ENTER" }) do mp.add_forced_key_binding(k, "spine-ok-" .. k, function() confirm(); render() end) end
mp.add_forced_key_binding("ESC", "spine-back-ESC", back)

-- Show the controls briefly on pause/seek/new file; keep them fresh while visible.
mp.observe_property("pause", "bool", function() wake(); render() end)
mp.register_event("seek", function() wake(1.5) end)
mp.register_event("file-loaded", function() wake(3) end)

-- Automatic skip: jump over an intro once, as you enter it.
mp.observe_property("chapter", "number", function(_, i)
  if SKIP ~= "auto" or i == nil then return end
  local part = current_part()
  if part and part.kind == "intro" and not skipped[i] then
    skipped[i] = true
    do_skip(part)
    mp.osd_message("Skipped intro", 2)
  end
end)

local shown = false
mp.add_periodic_timer(0.2, function()
  if SKIP == "auto" then
    local p = current_part()
    if p and p.web and p.kind ~= "credits" and not skipped["w" .. p.from] then
      skipped["w" .. p.from] = true
      do_skip(p)
      mp.osd_message(p.kind == "recap" and "Skipped recap" or "Skipped intro", 2)
    end
  end
  local paused = mp.get_property_bool("pause")
  local want = paused or menu or drag or upnext ~= nil or now() < visible_until or ((SKIP ~= "off") and current_part() ~= nil)
  if want or shown then render() end
  shown = want
end)
