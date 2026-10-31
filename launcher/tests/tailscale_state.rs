//! Cas 7 : parsing de sorties Tailscale fictives -> état et message. Aucune commande réelle n'est lancée.
#![allow(dead_code)]

#[path = "../src/tailscale.rs"]
mod tailscale;

use tailscale::{MSG_DISCONNECTED, MSG_NOT_INSTALLED, MSG_UNREACHABLE, TailscaleState, message, parse_backend_state, parse_magic_dns_suffix, server_host};

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
    // None => jamais de faux « connecté » issu d'un JSON cassé.
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
fn magic_dns_suffix_gives_the_server_host() {
    let j = r#"{"BackendState":"Running","MagicDNSSuffix":"tail1234.ts.net"}"#;
    assert_eq!(server_host("classroom", parse_magic_dns_suffix(j).as_deref()), "classroom.tail1234.ts.net");
    assert_eq!(server_host("classroom", parse_magic_dns_suffix(&status("Running")).as_deref()), "classroom");
}

#[test]
fn module_never_prints_cli_output() {
    // Aucune sortie de la CLI (noms de machines, IP) n'est loggée : pas de macro d'affichage dans le module.
    let src = include_str!("../src/tailscale.rs");
    for mac in ["println!", "eprintln!", "print!(", "eprint!", "dbg!", "log::", "tracing::"] {
        assert!(!src.contains(mac), "macro d'affichage trouvée : {mac}");
    }
}
