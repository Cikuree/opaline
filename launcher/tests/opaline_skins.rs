//! Cas 3 : import de skins (PNG 64×64 / 64×32 acceptés, le reste refusé), garde-robe (rename, delete,
//! index corrompu), multipart de `apply_skin` vers un serveur de fixtures, aucune requête sans jeton.
#![allow(dead_code)]
mod opaline_common;
mod support;

use opaline_common::*;
use opaline_core::skins::{self, Endpoints, Wardrobe};
use serde_json::Value;
use std::fs;
use support::{Mode, Server};

/// PNG RGBA uni de taille `w`×`h`.
fn png(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut wr = e.write_header().unwrap();
    wr.write_image_data(&vec![seed; (w * h * 4) as usize]).unwrap();
    drop(wr);
    out
}

fn wardrobe(t: &Tmp) -> Wardrobe {
    Wardrobe::new(t.0.join("skins"))
}

#[test]
fn png_64x64_and_64x32_are_accepted() {
    assert_eq!(skins::validate_png(&png(64, 64, 1)).unwrap(), (64, 64));
    assert_eq!(skins::validate_png(&png(64, 32, 1)).unwrap(), (64, 32));
    let t = Tmp::new("sk-ok");
    let w = wardrobe(&t);
    let a = w.import("Classique", "classic", &png(64, 64, 1)).unwrap();
    let b = w.import("Fin", "slim", &png(64, 32, 2)).unwrap();
    assert!(a.data_url.starts_with("data:image/png;base64,") && b.data_url.starts_with("data:image/png;base64,"));
    assert_eq!(w.list().len(), 2);
}

#[test]
fn bad_inputs_are_refused_with_clear_messages() {
    let t = Tmp::new("sk-bad");
    let w = wardrobe(&t);

    let big = skins::validate_png(&png(128, 128, 1)).unwrap_err().to_string();
    assert!(big.contains("128×128") && big.contains("64×64"), "{big}");
    assert!(w.import("x", "classic", &png(128, 128, 1)).is_err());

    // PNG corrompu : en-tête valide, données altérées (CRC ou zlib invalide).
    let mut corrupt = png(64, 64, 3);
    let mid = corrupt.len() / 2;
    for b in &mut corrupt[mid..mid + 16] {
        *b ^= 0xA5;
    }
    let msg = skins::validate_png(&corrupt).unwrap_err().to_string();
    assert!(msg.contains("PNG valide") || msg.contains("dimensions"), "{msg}");

    // Fichier non PNG renommé en .png (GIF, puis JPEG).
    let gif = b"GIF89a\x40\x00\x40\x00\x00\x00\x00;".to_vec();
    assert!(w.import("gif", "classic", &gif).unwrap_err().to_string().contains("PNG valide"));
    let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0, 1];
    assert!(w.import("jpg", "classic", &jpg).is_err());

    // Modèle inconnu et taille excessive.
    assert!(w.import("x", "giant", &png(64, 64, 1)).unwrap_err().to_string().contains("classic ou slim"));
    let too_big = vec![0u8; skins::MAX_SKIN_BYTES + 1];
    assert!(skins::validate_png(&too_big).unwrap_err().to_string().contains("256 Ko"));

    assert!(w.list().is_empty(), "aucun skin ne doit être enregistré après un refus");
}

#[test]
fn rename_delete_and_active_flag() {
    let t = Tmp::new("sk-garde");
    let w = wardrobe(&t);
    let s = w.import("  Mon\u{7} skin  ", "slim", &png(64, 64, 1)).unwrap();
    assert_eq!(s.name, "Mon skin", "caractères de contrôle retirés");
    w.adopt_current(&png(64, 32, 2), "classic").unwrap();
    w.rename(&s.id, &"N".repeat(40)).unwrap();
    assert_eq!(w.list().iter().find(|x| x.id == s.id).unwrap().name.chars().count(), 24, "nom tronqué à 24 caractères");
    assert!(w.rename("../etc", "x").is_err(), "identifiant hors format refusé");

    let active = w.list().into_iter().find(|x| x.active).unwrap();
    w.delete(&active.id).unwrap();
    assert!(w.list().iter().all(|x| !x.active), "suppression du skin actif : plus d'actif");
    assert_eq!(w.list().len(), 1);
    assert!(w.delete(&active.id).is_err(), "double suppression refusée");
    assert!(!t.0.join("skins").join(format!("{}.png", active.id)).exists());
}

#[test]
fn corrupt_index_falls_back_to_empty_wardrobe_without_panic() {
    let t = Tmp::new("sk-idx");
    let w = wardrobe(&t);
    w.import("A", "classic", &png(64, 64, 1)).unwrap();
    let idx = t.0.join("skins").join("index.json");
    for garbage in ["{pas du json", "", "[1,2,3]", "{\"skins\":\"pas une liste\"}"] {
        fs::write(&idx, garbage).unwrap();
        assert!(w.list().is_empty(), "index corrompu : repli sur une garde-robe vide ({garbage:?})");
        assert!(w.rename("0123456789abcdef", "x").is_err());
    }
    // Un import repart d'un index propre et réécrit le fichier.
    let s = w.import("B", "slim", &png(64, 64, 9)).unwrap();
    assert_eq!(w.list().iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), [s.id.as_str()]);
    assert!(serde_json::from_slice::<Value>(&fs::read(&idx).unwrap()).is_ok());
}

#[test]
fn apply_skin_sends_multipart_with_variant_then_file_and_bearer() {
    let _g = env_lock();
    let srv = Server::start();
    srv.put("/minecraft/profile/skins", "{}");
    let ep = Endpoints::with_base(&srv.base);
    let p = png(64, 64, 7);
    skins::apply_skin(&http(), &ep, "tok-SECRET-123", "slim", &p).expect("envoi");

    let r = srv.requests().into_iter().find(|r| r.path == "/minecraft/profile/skins").expect("requête");
    assert_eq!(r.method, "POST");
    assert_eq!(r.auth.as_deref(), Some("Bearer tok-SECRET-123"));
    let body = &r.body;
    let text = String::from_utf8_lossy(body).into_owned();
    let v = text.find("name=\"variant\"").expect("champ variant");
    let f = text.find("name=\"file\"; filename=\"skin.png\"").expect("champ file");
    assert!(v < f, "variant doit précéder file");
    assert!(text[v..].starts_with("name=\"variant\"\r\n\r\nslim\r\n"));
    assert!(body.windows(p.len()).any(|w| w == p.as_slice()), "les octets PNG sont envoyés tels quels");
}

#[test]
fn apply_skin_http_errors_are_clean_and_hide_token() {
    let _g = env_lock();
    let srv = Server::start();
    let ep = Endpoints::with_base(&srv.base);
    let p = png(64, 64, 7);
    srv.put_mode("/minecraft/profile/skins", "", Mode::Status(401));
    let e = skins::apply_skin(&http(), &ep, "tok-SECRET-123", "classic", &p).unwrap_err().to_string();
    assert!(e.contains("session expirée"), "{e}");
    assert!(!e.contains("tok-SECRET"), "{e}");
    srv.put_mode("/minecraft/profile/skins", "", Mode::Status(429));
    assert!(skins::apply_skin(&http(), &ep, "t", "classic", &p).unwrap_err().to_string().contains("trop de requêtes"));
}

#[test]
fn no_request_is_sent_without_token() {
    let _g = env_lock();
    let srv = Server::start();
    let ep = Endpoints::with_base(&srv.base);
    let c = http();
    let p = png(64, 64, 1);
    assert!(skins::get_profile(&c, &ep, "").unwrap_err().to_string().contains("requise"));
    assert!(skins::get_profile(&c, &ep, "   ").is_err());
    assert!(skins::apply_skin(&c, &ep, "", "classic", &p).is_err());
    assert!(skins::set_cape(&c, &ep, "", None).is_err());
    assert!(skins::set_cape(&c, &ep, "", Some("0123")).is_err());
    let t = Tmp::new("sk-notok");
    let w = wardrobe(&t);
    w.import("A", "classic", &p).unwrap();
    assert!(skins::sync_current(&c, &ep, "", &w).is_err());
    assert!(srv.requests().is_empty(), "requêtes sans jeton : {:?}", srv.requests().iter().map(|r| &r.path).collect::<Vec<_>>());
}

#[test]
fn profile_capes_and_skin_parsing_from_fixture() {
    let _g = env_lock();
    let srv = Server::start();
    srv.put(
        "/minecraft/profile",
        r#"{"skins":[{"state":"ACTIVE","url":"https://textures.minecraft.net/texture/b","variant":"SLIM"}],
        "capes":[{"id":"c1","state":"ACTIVE","url":"https://textures.minecraft.net/cape","alias":"Migrator"},{"id":"c2","state":"INACTIVE","url":"https://evil.example/x","alias":"Y"}]}"#,
    );
    let ep = Endpoints::with_base(&srv.base);
    let prof = skins::get_profile(&http(), &ep, "tok").unwrap();
    assert_eq!(skins::current_skin(&prof), Some(("https://textures.minecraft.net/texture/b".into(), "slim")));
    let capes = skins::parse_capes(&prof);
    assert_eq!(capes.len(), 2);
    assert!(capes[0].active && !capes[1].active);
    assert_eq!(capes[1].url, "", "URL de cape hors hôte officiel vidée");
}
