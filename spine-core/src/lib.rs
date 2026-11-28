//! Spine core: scan a ROM folder, clean up the names, group duplicates,
//! and work out how to launch each game.
//!
//! The desktop app (Tauri) calls into this crate; the `spine-scan` command
//! line tool exercises it on its own.

pub mod addons;
pub mod aniskip;
pub mod backup;
pub mod bios;
pub mod config;
pub mod covers;
pub mod emus;
pub mod extras;
pub mod flathub;
pub mod hostenv;
pub mod launch;
pub mod meta;
pub mod movies;
pub mod mpvget;
pub mod people;
pub mod player;
pub mod ports;
pub mod scan;
pub mod stores;
pub mod retroachievements;
pub mod screenscraper;
pub mod systems;
pub mod theme_import;
pub mod themes;
pub mod title;
pub mod tmdb_stream;
pub mod tv;

pub use scan::{scan, Game, Library, SystemLibrary};
