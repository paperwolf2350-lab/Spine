//! Spine core: scan a ROM folder, clean up the names, group duplicates,
//! and work out how to launch each game.
//!
//! The desktop app (Tauri) calls into this crate; the `spine-scan` command
//! line tool exercises it on its own.

pub mod addons;
pub mod aniskip;
pub mod bios;
pub mod config;
pub mod covers;
pub mod emus;
pub mod extras;
pub mod hostenv;
pub mod launch;
pub mod meta;
pub mod movies;
pub mod mpvget;
pub mod people;
pub mod player;
pub mod scan;
pub mod stores;
pub mod retroachievements;
pub mod screenscraper;
pub mod systems;
pub mod title;
pub mod tv;

pub use scan::{scan, Game, Library, SystemLibrary};
