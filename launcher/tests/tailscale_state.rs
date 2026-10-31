//! Cas 7 : parsing de sorties Tailscale fictives -> état et message. Aucune commande réelle n'est lancée.
#![allow(dead_code)]

#[path = "../src/tailscale.rs"]
mod tailscale;

use tailscale::{MSG_DISCONNECTED, MSG_NOT_INSTALLED, MSG_UNREACHABLE, TailscaleState, has_tailscale_addr, message, parse_backend_state};
use std::net::Ipv4Addr;

fn status(backend: &str) -> String {
    format!(r#"{{"Version":"1.80.0","BackendState":"{backend}","Self":{{"HostName":"poste-eleve-07","TailscaleIPs":["100.101.2.3"]}},"Peer":{{}}}}"#)
}

#[test]
fn running_is_connected() {
    assert_eq!(parse_backend_state(&status("Running")), Some(TailscaleState::Connected));
    assert_eq!(message(TailscaleState::Connected, true), None);
}

#[test]
fn stopped_and_needs_login_are_disconnected_with_hint() {
    for s in ["Stopped", "NeedsLogin", "NoState", "Starting"] {
        let st = parse_backend_state(&status(s)).unwrap_or_else(|| panic!("{s} : pas d'état"));
        assert_eq!(st, TailscaleState::Disconnected, "{s}");
        assert_eq!(message(st, false), Some(MSG_DISCONNECTED), "{s}");
    }
}

#[test]
fn invalid_or_incomplete_json_gives_no_state() {
    // None => le module passe au repli par adresse (pas de faux « connecté » issu d'un JSON cassé).
    assert_eq!(parse_backend_state("pas du json"), None);
    assert_eq!(parse_backend_state(""), None);
    assert_eq!(parse_backend_state("{}"), None);
    assert_eq!(parse_backend_state(r#"{"BackendState":5}"#), None);
    assert_eq!(parse_backend_state(r#"{"BackendState":"Running""#), None, "JSON tronqué");
}

#[test]
fn not_installed_message() {
    assert_eq!(message(TailscaleState::NotInstalled, false), Some(MSG_NOT_INSTALLED));
    assert_eq!(message(TailscaleState::NotInstalled, true), Some(MSG_NOT_INSTALLED));
}

#[test]
fn connected_but_server_unreachable_message() {
    assert_eq!(message(TailscaleState::Connected, false), Some(MSG_UNREACHABLE));
}

#[test]
fn fallback_address_detection_on_ipconfig_and_ip_output() {
    // Repli quand la CLI est absente de la sortie : une adresse CGNAT Tailscale suffit.
    assert!(has_tailscale_addr("   Adresse IPv4. . . . . . . . . . . : 100.101.2.3\n"));
    assert!(has_tailscale_addr("2: tailscale0    inet 100.64.0.1/32 scope global tailscale0"));
    assert!(!has_tailscale_addr("   Adresse IPv4 : 192.168.1.20\n   Passerelle : 10.0.0.1"));
    assert!(!has_tailscale_addr("inet 100.63.255.255 inet 100.128.0.1"));
    assert!(!has_tailscale_addr(""));
}

#[test]
fn cgnat_boundaries() {
    use tailscale::is_tailscale_ip;
    assert!(is_tailscale_ip(Ipv4Addr::new(100, 64, 0, 0)));
    assert!(is_tailscale_ip(Ipv4Addr::new(100, 127, 255, 255)));
    assert!(!is_tailscale_ip(Ipv4Addr::new(100, 128, 0, 0)));
    assert!(!is_tailscale_ip(Ipv4Addr::new(100, 63, 255, 255)));
}

#[test]
fn module_never_prints_cli_output() {
    // Aucune sortie de la CLI (noms de machines, IP) n'est loggée : pas de macro d'affichage dans le module.
    let src = include_str!("../src/tailscale.rs");
    for mac in ["println!", "eprintln!", "print!(", "eprint!", "dbg!", "log::", "tracing::"] {
        assert!(!src.contains(mac), "macro d'affichage trouvée : {mac}");
    }
}
