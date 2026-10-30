//! credentials.txt: account details the user types in themselves, next to the
//! program. Simple "key = value" lines; '#' starts a comment.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const TEMPLATE: &str = "\
# Spine accounts. Fill these in, save, then run build-and-scan.bat again.
# This file stays on your PC: Spine only sends each value to the site it belongs to.

# SteamGridDB: sign in at https://www.steamgriddb.com, then
# Preferences > API, and copy the key here.
steamgriddb_key =

# ScreenScraper account (https://www.screenscraper.fr)
screenscraper_user = {USER}
screenscraper_password =

# ScreenScraper developer key for Spine (sent to you by the ScreenScraper team
# after you request one on their forum). Leave blank until you have it.
screenscraper_devid =
screenscraper_devpassword =

# RetroAchievements (https://retroachievements.org): your username, and the
# Web API key from Settings > Authentication on their site.
retroachievements_user =
retroachievements_key =
";

/// Spine's own ScreenScraper developer key, built into official releases by
/// GitHub Actions (secrets SCREENSCRAPER_DEVID / SCREENSCRAPER_DEVPASSWORD).
pub fn builtin_dev_key() -> Option<(&'static str, &'static str)> {
    let id = option_env!("SPINE_SS_DEVID").filter(|s| !s.trim().is_empty())?;
    let pw = option_env!("SPINE_SS_DEVPASSWORD").filter(|s| !s.trim().is_empty())?;
    Some((id.trim(), pw.trim()))
}

#[derive(Debug, Default, Clone)]
pub struct Credentials {
    pub steamgriddb_key: Option<String>,
    pub ss_user: Option<String>,
    pub ss_password: Option<String>,
    pub ss_devid: Option<String>,
    pub ss_devpassword: Option<String>,
    pub ra_user: Option<String>,
    pub ra_key: Option<String>,
}

impl Credentials {
    pub fn load(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else { return Self::default() };
        let mut m: HashMap<String, String> = HashMap::new();
        for line in text.lines() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches('"').to_string();
                if !v.is_empty() {
                    m.insert(k.trim().to_lowercase(), v);
                }
            }
        }
        Credentials {
            steamgriddb_key: m.remove("steamgriddb_key"),
            ss_user: m.remove("screenscraper_user"),
            ss_password: m.remove("screenscraper_password"),
            ss_devid: m.remove("screenscraper_devid"),
            ss_devpassword: m.remove("screenscraper_devpassword"),
            ra_user: m.remove("retroachievements_user"),
            ra_key: m.remove("retroachievements_key"),
        }
    }

    /// The developer key to use: the one typed in Settings, else the one built into
    /// official releases (from the repository's secrets; never in the source code).
    pub fn dev_key(&self) -> Option<(String, String)> {
        match (&self.ss_devid, &self.ss_devpassword) {
            (Some(i), Some(p)) => Some((i.clone(), p.clone())),
            _ => builtin_dev_key().map(|(i, p)| (i.to_string(), p.to_string())),
        }
    }

    /// ScreenScraper can be used: a developer key is available. A user account
    /// (username + password) is optional but gives a bigger daily allowance.
    pub fn screenscraper_ready(&self) -> bool {
        self.dev_key().is_some() && (self.ss_user.is_some() == self.ss_password.is_some())
    }

    /// Save all fields back to the file (keeps the explanatory comments).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let v = |o: &Option<String>| o.clone().unwrap_or_default();
        let mut text = String::new();
        for line in TEMPLATE.replace("{USER}", "").lines() {
            let out = match line.split_once('=').map(|(k, _)| k.trim()) {
                Some("steamgriddb_key") => format!("steamgriddb_key = {}", v(&self.steamgriddb_key)),
                Some("screenscraper_user") => format!("screenscraper_user = {}", v(&self.ss_user)),
                Some("screenscraper_password") => format!("screenscraper_password = {}", v(&self.ss_password)),
                Some("screenscraper_devid") => format!("screenscraper_devid = {}", v(&self.ss_devid)),
                Some("screenscraper_devpassword") => format!("screenscraper_devpassword = {}", v(&self.ss_devpassword)),
                Some("retroachievements_user") => format!("retroachievements_user = {}", v(&self.ra_user)),
                Some("retroachievements_key") => format!("retroachievements_key = {}", v(&self.ra_key)),
                _ => line.replace("Fill these in, save, then run build-and-scan.bat again.", "Spine edits this file from Settings > Scrapers."),
            };
            text.push_str(&out);
            text.push_str("\r\n");
        }
        fs::write(path, text)
    }

    /// Write a blank template if the file doesn't exist yet.
    pub fn ensure_template(path: &Path, user: &str) {
        if !path.exists() {
            let _ = fs::write(path, TEMPLATE.replace("{USER}", user).replace('\n', "\r\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_ignores_blanks() {
        let dir = std::env::temp_dir().join("spine-cred-test");
        let _ = fs::create_dir_all(&dir);
        let p = dir.join("credentials.txt");
        let _ = fs::remove_file(&p);
        Credentials::ensure_template(&p, "example_user");
        let c = Credentials::load(&p);
        assert_eq!(c.ss_user.as_deref(), Some("example_user"));
        assert!(c.ss_password.is_none() && c.steamgriddb_key.is_none());
        assert!(!c.screenscraper_ready());
        fs::write(&p, "steamgriddb_key = abc123\nscreenscraper_user=example_user\nscreenscraper_password = p w\nscreenscraper_devid=d\nscreenscraper_devpassword=dp\n").unwrap();
        let c = Credentials::load(&p);
        assert_eq!(c.steamgriddb_key.as_deref(), Some("abc123"));
        assert_eq!(c.ss_password.as_deref(), Some("p w"));
        assert!(c.screenscraper_ready());
        let mut c2 = c.clone();
        c2.steamgriddb_key = Some("newkey".into());
        c2.ss_password = None;
        c2.save(&p).unwrap();
        let c3 = Credentials::load(&p);
        assert_eq!(c3.steamgriddb_key.as_deref(), Some("newkey"));
        assert_eq!(c3.ss_user.as_deref(), Some("example_user"));
        assert!(c3.ss_password.is_none());
    }
}
