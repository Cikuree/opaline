//! Cas 6 : amis. Fusion des échantillons de ping (deux packs, mêmes joueurs), dates `lastSeen` ISO 8601,
//! cache corrompu, et `friends_list` de bout en bout contre un faux serveur SLP local (aucun réseau réel).
#![allow(dead_code)]
mod opaline_common;
mod support;

use opaline_common::*;
use opaline_core::config::Dirs;
use opaline_core::friends::{self, Friend};
use opaline_core::ping;
use opaline_core::service;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use support::dead_base;

fn uuid(n: u8) -> String {
    format!("{n:032x}")
}

fn names(v: &[Friend]) -> Vec<(&str, bool)> {
    v.iter().map(|f| (f.name.as_str(), f.online)).collect()
}

#[test]
fn samples_from_several_packs_are_merged_by_uuid() {
    let t = Tmp::new("fr-merge");
    let d = Dirs::new(t.0.join("root"));
    let dashed = "00000000-0000-0000-0000-000000000001";
    // Même joueur, une fois avec tirets et une fois sans : une seule entrée.
    let seen = vec![("Alice".to_string(), dashed.to_string()), ("Alice".to_string(), uuid(1)), ("Bob".to_string(), uuid(2))];
    let out = friends::record(&d, &seen, 1_709_164_800);
    assert_eq!(out.len(), 2, "{out:?}");
    assert!(out.iter().all(|f| f.online && f.in_game), "vus à ce relevé : en ligne et en jeu");
    assert_eq!(out[0].uuid, dashed);
    assert!(out.iter().any(|f| f.uuid == "00000000-0000-0000-0000-000000000002"));
}

#[test]
fn last_seen_is_iso8601_utc_and_remembered_offline() {
    let t = Tmp::new("fr-iso");
    let d = Dirs::new(t.0.join("root"));
    friends::record(&d, &[("Zed".into(), uuid(1))], 1_709_164_800 + 3661);
    let later = friends::record(&d, &[("Bob".into(), uuid(2))], 1_709_164_800 + 86_400 * 2);
    let zed = later.iter().find(|f| f.name == "Zed").unwrap();
    assert!(!zed.online && !zed.in_game, "absent du dernier relevé : hors ligne");
    assert_eq!(zed.last_seen.as_deref(), Some("2024-02-29T01:01:01Z"));
    let bob = later.iter().find(|f| f.name == "Bob").unwrap();
    assert_eq!(bob.last_seen.as_deref(), Some("2024-03-02T00:00:00Z"));
    for f in &later {
        let s = f.last_seen.as_deref().unwrap();
        let b = s.as_bytes();
        assert!(s.len() == 20 && b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':' && b[16] == b':' && b[19] == b'Z', "{s}");
    }
    assert_eq!(friends::iso8601(0), "1970-01-01T00:00:00Z");
    assert_eq!(friends::iso8601(951_782_400), "2000-02-29T00:00:00Z", "année bissextile");
}

#[test]
fn order_is_online_first_then_name_case_insensitive() {
    let t = Tmp::new("fr-order");
    let d = Dirs::new(t.0.join("root"));
    friends::record(&d, &[("zed".into(), uuid(1)), ("Alice".into(), uuid(2))], 100);
    let out = friends::record(&d, &[("zed".into(), uuid(1)), ("bob".into(), uuid(3))], 200);
    assert_eq!(names(&out), [("bob", true), ("zed", true), ("Alice", false)]);
}

#[test]
fn invalid_samples_are_skipped() {
    let t = Tmp::new("fr-invalid");
    let d = Dirs::new(t.0.join("root"));
    let seen = vec![("trop-long-pour-un-pseudo".into(), uuid(1)), (String::new(), uuid(2)), ("Ok".into(), "pas-un-uuid".into()), ("Valide".into(), uuid(4))];
    let out = friends::record(&d, &seen, 10);
    assert_eq!(names(&out), [("Valide", true)]);
}

#[test]
fn cache_is_capped_and_keeps_most_recent() {
    let t = Tmp::new("fr-cap");
    let d = Dirs::new(t.0.join("root"));
    let batch: Vec<(String, String)> = (1..=500u32).map(|n| (format!("p{n}"), format!("{n:032x}"))).collect();
    friends::record(&d, &batch, 1);
    let out = friends::record(&d, &[("Nouveau".into(), format!("{:032x}", 0xffffu32))], 2);
    assert_eq!(out.len(), 500, "plafond de 500 entrées");
    assert!(out.iter().any(|f| f.name == "Nouveau"));
}

#[test]
fn corrupt_cache_is_ignored_and_rewritten() {
    let t = Tmp::new("fr-corrupt");
    let d = Dirs::new(t.0.join("root"));
    fs::create_dir_all(d.cache()).unwrap();
    for garbage in ["{pas du json", "", "[1,2]", "{\"players\":\"oops\"}"] {
        fs::write(d.cache().join("friends.json"), garbage).unwrap();
        let out = friends::record(&d, &[("Alice".into(), uuid(1))], 5);
        assert_eq!(names(&out), [("Alice", true)], "cache corrompu ({garbage:?})");
        let b = fs::read(d.cache().join("friends.json")).unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&b).is_ok(), "cache réécrit valide");
    }
}

/// Réponse SLP : `status` est lue à chaque requête de statut, ce qui permet de changer l'échantillon entre deux appels.
fn slp_server(status: Arc<Mutex<String>>) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let st = status.clone();
            std::thread::spawn(move || slp_conn(s, st));
        }
    });
    port
}

fn slp_conn(mut s: TcpStream, st: Arc<Mutex<String>>) {
    while let Ok(len) = ping::read_varint(&mut s) {
        let mut buf = vec![0u8; len.max(0) as usize];
        if s.read_exact(&mut buf).is_err() {
            return;
        }
        let mut r = &buf[..];
        let Ok(id) = ping::read_varint(&mut r) else { return };
        let body: Vec<u8> = match id {
            // Requête de statut : id 0 sans corps (le handshake a un corps).
            0 if r.is_empty() => {
                let json = st.lock().unwrap().clone();
                let mut b = Vec::new();
                ping::write_varint(&mut b, json.len() as i32);
                b.extend_from_slice(json.as_bytes());
                b
            }
            // Ping : écho du corps.
            1 => r.to_vec(),
            _ => continue,
        };
        let mut payload = Vec::new();
        ping::write_varint(&mut payload, id);
        payload.extend_from_slice(&body);
        let mut out = Vec::new();
        ping::write_varint(&mut out, payload.len() as i32);
        out.extend_from_slice(&payload);
        if s.write_all(&out).is_err() {
            return;
        }
    }
}

#[test]
fn friends_list_end_to_end_merges_packs_and_keeps_memory_when_offline() {
    let _g = env_lock();
    let status = Arc::new(Mutex::new(String::new()));
    let both = r#"{"version":{"name":"26.3","protocol":777},"players":{"max":8,"online":2,"sample":[{"name":"Alice","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"},{"name":"Bob","id":"11111111111111111111111111111111"},{"name":"Fantome","id":""}]},"description":{"text":"Classroom Run"}}"#;
    let alice_only = r#"{"version":{"name":"26.3","protocol":777},"players":{"max":8,"online":1,"sample":[{"name":"Alice","id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}]},"description":{"text":"Classroom Run"}}"#;
    *status.lock().unwrap() = both.to_string();
    let port = slp_server(status.clone());

    let t = Tmp::new("fr-e2e");
    let root = Dirs::new(t.0.join("root"));
    let f = t.0.join("packs.json");
    let entry = |id: &str| {
        format!(
            r#"{{"id":"{id}","name":"{id}","mc":"26.3","loader":"fabric","pack":"https://github.com/x/p.zip","packHash":"abcdef0123456789","server":{{"host":"classroom","port":25565,"motd":"Classroom Run"}}}}"#
        )
    };
    fs::write(&f, format!(r#"{{"packs":[{},{}]}}"#, entry("alpha"), entry("beta"))).unwrap();
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    set_env("CLASSROOM_SERVER", &format!("127.0.0.1:{port}"));

    // Deux packs sur le même serveur : chaque joueur n'apparaît qu'une fois.
    let first = service::friends_list(&root).expect("liste");
    assert_eq!(names(&first), [("Alice", true), ("Bob", true)], "{first:?}");
    assert!(first.iter().all(|f| f.last_seen.as_deref().is_some_and(|s| s.ends_with('Z') && s.contains('T'))));

    // Bob n'est plus dans l'échantillon : il reste mémorisé, hors ligne.
    *status.lock().unwrap() = alice_only.to_string();
    let second = service::friends_list(&root).unwrap();
    assert_eq!(names(&second), [("Alice", true), ("Bob", false)]);
    assert_eq!(second[1].last_seen, first[1].last_seen, "date de dernière visite conservée");

    // Serveur hors ligne : pas d'erreur, la mémoire reste affichée.
    set_env("CLASSROOM_SERVER", dead_base().trim_start_matches("http://"));
    let offline = service::friends_list(&root).expect("hors ligne sans erreur");
    assert_eq!(names(&offline), [("Alice", false), ("Bob", false)]);

    unset_env("CLASSROOM_SERVER");
    unset_env("OPALINE_PACKS_URL");
}
