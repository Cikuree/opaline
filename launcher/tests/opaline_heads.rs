//! Cas 4 : URL de texture (hôte officiel uniquement), recadrage visage + chapeau, cache des têtes.
//! Aucun téléchargement réel : les profils de fixtures ne pointent jamais vers `textures.minecraft.net`.
#![allow(dead_code)]
mod opaline_common;
mod support;

use base64::Engine;
use opaline_common::*;
use opaline_core::config::Dirs;
use opaline_core::head::{crop_head, data_url, normalize_uuid, player_head, skin_url_from_profile};
use opaline_core::skins::{fetch_texture, is_texture_url, official_texture_url};
use serde_json::{Value, json};
use std::fs;
use std::time::{Duration, SystemTime};
use support::{Server, dead_base};

const UUID: &str = "b50ad385-829d-3141-a216-7e7d7539ba7f";
const UUID_HEX: &str = "b50ad385829d3141a2167e7d7539ba7f";

/// Profil Mojang minimal dont la propriété `textures` pointe vers `url`.
fn profile_with_skin_url(url: &str) -> Value {
    let t = base64::engine::general_purpose::STANDARD.encode(format!(r#"{{"textures":{{"SKIN":{{"url":"{url}"}}}}}}"#));
    json!({"id": UUID_HEX, "name": "Alice", "properties": [{"name": "textures", "value": t}]})
}

fn decode_rgba(png_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut r = png::Decoder::new(std::io::Cursor::new(png_bytes)).read_info().unwrap();
    let mut b = vec![0; r.output_buffer_size().unwrap()];
    let f = r.next_frame(&mut b).unwrap();
    (f.width, f.height, b[..f.buffer_size()].to_vec())
}

/// Skin 64×h : visage (8,8) rouge opaque ; chapeau (40,8) : colonne x=0 vert alpha 128, reste transparent.
fn skin(h: u32) -> Vec<u8> {
    let mut px = vec![0u8; (64 * h * 4) as usize];
    for y in 0..8 {
        for x in 0..8 {
            let f = (((8 + y) * 64 + 8 + x) * 4) as usize;
            px[f..f + 4].copy_from_slice(&[200, 0, 0, 255]);
            let g = (((8 + y) * 64 + 40 + x) * 4) as usize;
            if x == 0 {
                px[g..g + 4].copy_from_slice(&[0, 200, 0, 128]);
            }
        }
    }
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, 64, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&px).unwrap();
    out
}

#[test]
fn only_the_official_texture_host_is_accepted() {
    for ok in ["https://textures.minecraft.net/texture/abc", "http://textures.minecraft.net/texture/abc", "https://TEXTURES.minecraft.net/texture/abc"] {
        let u = official_texture_url(ok).unwrap_or_else(|| panic!("refusée à tort : {ok}"));
        assert!(u.starts_with("https://textures.minecraft.net/"), "{u}");
    }
    let refused = [
        "https://textures.minecraft.net.evil.com/texture/x",
        "https://evil.com/textures.minecraft.net/x",
        "https://textures.minecraft.net@evil.com/texture/x",
        "https://user@textures.minecraft.net/texture/x",
        "https://textures.minecraft.net:8443/texture/x",
        "https://textures.minecraft.net.evil.com:443/x",
        "ftp://textures.minecraft.net/x",
        "file:///etc/passwd",
        "textures.minecraft.net/texture/x",
        "",
    ];
    for bad in refused {
        assert!(official_texture_url(bad).is_none(), "acceptée à tort : {bad}");
        assert!(!is_texture_url(bad), "is_texture_url : {bad}");
    }
    // is_texture_url est strict : pas de http, pas d'hôte piégé.
    assert!(is_texture_url("https://textures.minecraft.net/texture/abc"));
    assert!(!is_texture_url("http://textures.minecraft.net/texture/abc"));
}

#[test]
fn backslash_trick_still_resolves_to_official_host() {
    // WHATWG traite « \ » comme « / » : l'hôte reste textures.minecraft.net, la sortie est reconstruite.
    let u = official_texture_url("https://textures.minecraft.net\\@evil.com/texture/x").expect("hôte officiel");
    assert_eq!(reqwest::Url::parse(&u).unwrap().host_str(), Some("textures.minecraft.net"));
}

#[test]
fn profile_texture_url_is_checked_before_anything_else() {
    assert_eq!(skin_url_from_profile(&profile_with_skin_url("http://textures.minecraft.net/texture/y")).unwrap(), "https://textures.minecraft.net/texture/y");
    for bad in ["https://evil.example/texture/x", "https://textures.minecraft.net.evil.com/x", "https://textures.minecraft.net@evil.example/x"] {
        assert!(skin_url_from_profile(&profile_with_skin_url(bad)).is_err(), "{bad}");
    }
    assert!(skin_url_from_profile(&json!({})).is_err());
}

#[test]
fn fetch_texture_refuses_foreign_host_without_any_request() {
    let srv = Server::start();
    let err = fetch_texture(&http(), &srv.url("/texture/x")).unwrap_err().to_string();
    assert!(err.contains("hôte non officiel"), "{err}");
    assert!(srv.requests().is_empty());
}

#[test]
fn crop_face_and_hat_overlay_on_64x64_and_64x32() {
    for h in [64, 32] {
        let (w, hh, px) = decode_rgba(&crop_head(&skin(h)).unwrap());
        assert_eq!((w, hh), (8, 8));
        // (0,0) : chapeau vert alpha 128 sur visage rouge -> R=99, G=100 (arrondis entiers).
        assert_eq!(&px[..4], &[99, 100, 0, 255], "mélange partiel du chapeau");
        // (1,0) : chapeau transparent -> visage pur.
        assert_eq!(&px[4..8], &[200, 0, 0, 255], "visage seul");
        assert_eq!(px.len(), 8 * 8 * 4);
    }
}

#[test]
fn crop_rejects_wrong_sizes_and_garbage() {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, 128, 128);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&vec![0u8; 128 * 128 * 4]).unwrap();
    assert!(crop_head(&out).is_err(), "128×128 refusé");
    assert!(crop_head(b"pas une image").is_err());
}

#[test]
fn uuid_is_normalized_and_bad_values_are_refused() {
    assert_eq!(normalize_uuid(UUID).unwrap(), UUID_HEX);
    assert_eq!(normalize_uuid(&UUID.to_uppercase()).unwrap(), UUID_HEX);
    assert!(normalize_uuid("../../etc").is_err());
    assert!(normalize_uuid("zz").is_err());
}

#[test]
fn fresh_cache_is_served_without_network() {
    let t = Tmp::new("head-fresh");
    let dirs = Dirs::new(t.0.join("root"));
    let file = dirs.cache().join("heads").join(format!("{UUID_HEX}.png"));
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let head = crop_head(&skin(64)).unwrap();
    fs::write(&file, &head).unwrap();
    // Forme avec tirets : même entrée de cache, réseau mort.
    let url = player_head(&dirs, &http(), UUID, &dead_base()).expect("cache frais");
    assert_eq!(url, data_url(&head));
    assert!(url.starts_with("data:image/png;base64,"));
}

#[test]
fn stale_cache_is_used_when_network_fails() {
    let t = Tmp::new("head-stale");
    let dirs = Dirs::new(t.0.join("root"));
    let file = dirs.cache().join("heads").join(format!("{UUID_HEX}.png"));
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let head = crop_head(&skin(64)).unwrap();
    fs::write(&file, &head).unwrap();
    let old = SystemTime::now() - Duration::from_secs(3 * 24 * 3600);
    fs::File::options().write(true).open(&file).unwrap().set_modified(old).unwrap();

    let url = player_head(&dirs, &http(), UUID, &dead_base()).expect("cache périmé utilisé hors ligne");
    assert_eq!(url, data_url(&head));
}

#[test]
fn no_cache_and_no_network_is_a_clean_error() {
    let t = Tmp::new("head-none");
    let dirs = Dirs::new(t.0.join("root"));
    let err = player_head(&dirs, &http(), UUID, &dead_base()).unwrap_err().to_string();
    assert!(err.contains("réseau") || err.contains("profil"), "{err}");
    assert!(!dirs.cache().join("heads").exists());
    assert!(player_head(&dirs, &http(), "not-a-uuid", &dead_base()).unwrap_err().to_string().contains("UUID invalide"));
}

#[test]
fn profile_with_foreign_texture_is_refused_and_not_cached() {
    let srv = Server::start();
    srv.put(&format!("/session/minecraft/profile/{UUID_HEX}"), profile_with_skin_url("https://evil.example/texture/x").to_string());
    let t = Tmp::new("head-evil");
    let dirs = Dirs::new(t.0.join("root"));
    let err = player_head(&dirs, &http(), UUID, &srv.base).unwrap_err().to_string();
    assert!(err.contains("adresse de texture refusée"), "{err}");
    assert_eq!(srv.requests().len(), 1, "seul le profil est demandé ; aucun hôte externe contacté");
    assert!(!dirs.cache().join("heads").join(format!("{UUID_HEX}.png")).exists());
}

#[test]
fn bug_corrupt_head_cache_should_be_ignored_and_refetched() {
    let t = Tmp::new("head-bug");
    let dirs = Dirs::new(t.0.join("root"));
    let file = dirs.cache().join("heads").join(format!("{UUID_HEX}.png"));
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, b"<html>").unwrap();
    // Attendu : un cache invalide n'est pas renvoyé (erreur ou nouvelle récupération).
    let r = player_head(&dirs, &http(), UUID, &dead_base());
    assert!(r.map_or(true, |u| u != data_url(b"<html>")));
}
