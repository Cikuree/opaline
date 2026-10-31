//! `opaline_core` : cœur d'Opaline (installation multi-pack, lancement, authentification, skins, serveur).
//! Aucune interface graphique : le backend Tauri (`opaline/src-tauri`) appelle `service`.
pub mod auth;
pub mod catalog;
pub mod changelog;
pub mod config;
pub mod download;
pub mod fabric;
pub mod friends;
pub mod head;
pub mod install;
pub mod java;
pub mod launch;
pub mod mojang;
pub mod packwiz;
pub mod perf;
pub mod ping;
pub mod safe;
pub mod serverview;
pub mod service;
pub mod skins;
pub mod stats;
pub mod sysmem;
pub mod tailscale;
pub mod textures;
pub mod update;
