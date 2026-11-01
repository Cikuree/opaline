//! Détection locale de l'état de Tailscale. Rien de la sortie de la CLI (noms de machines, IP) n'est loggé ni conservé.
use serde_json::Value;
use std::io::Read;
use std::net::Ipv4Addr;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailscaleState {
    NotInstalled,
    Disconnected,
    Connected,
}

/// `BackendState` de `tailscale status --json` (champ de `ipnstate.Status`, valeur « Running » = connecté).
/// None si le JSON est invalide.
pub fn parse_backend_state(json: &str) -> Option<TailscaleState> {
    let v: Value = serde_json::from_str(json).ok()?;
    let s = v.get("BackendState")?.as_str()?;
    Some(if s == "Running" { TailscaleState::Connected } else { TailscaleState::Disconnected })
}

/// Suffixe MagicDNS du tailnet (`MagicDNSSuffix`, ou `CurrentTailnet.MagicDNSSuffix`). Calculé à l'exécution, jamais loggé.
/// Refusé s'il contient autre chose que lettres, chiffres, `.` et `-`.
pub fn parse_magic_dns_suffix(json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    let s = v
        .get("MagicDNSSuffix")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| v.pointer("/CurrentTailnet/MagicDNSSuffix").and_then(Value::as_str))?;
    let s = s.trim().trim_matches('.');
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')).then(|| s.to_string())
}

/// `base.suffixe` si le suffixe est connu, sinon `base` seul.
pub fn server_host(base: &str, suffix: Option<&str>) -> String {
    match suffix {
        Some(s) => format!("{base}.{s}"),
        None => base.to_string(),
    }
}

/// Exécute une commande avec un délai ; sortie standard seulement.
fn run(cmd: &mut Command, timeout: Duration) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        let _ = tx.send(s);
    });
    match rx.recv_timeout(timeout) {
        Ok(s) => child.wait().ok()?.success().then_some(s),
        Err(_) => {
            let _ = child.kill();
            None
        }
    }
}

fn cli_path() -> Option<std::path::PathBuf> {
    let default = std::path::PathBuf::from(r"C:\Program Files\Tailscale\tailscale.exe");
    if cfg!(windows) && default.is_file() {
        return Some(default);
    }
    let name = if cfg!(windows) { "tailscale.exe" } else { "tailscale" };
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|f| f.is_file()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct TailscaleInfo {
    pub state: TailscaleState,
    pub magic_dns_suffix: Option<String>,
}

/// Une seule exécution de `tailscale status --json` (délai 3 s) : état et suffixe MagicDNS.
pub fn status() -> TailscaleInfo {
    snapshot().0
}

/// Une seule exécution de `tailscale status --json` : état, suffixe MagicDNS et IPv4 des pairs en ligne (vide si non connecté).
pub fn snapshot() -> (TailscaleInfo, Vec<Ipv4Addr>) {
    let Some(cli) = cli_path() else {
        return (TailscaleInfo { state: TailscaleState::NotInstalled, magic_dns_suffix: None }, Vec::new());
    };
    // CLI présente mais en échec (service arrêté, délai dépassé) : non connecté.
    let Some(json) = run(Command::new(cli).args(["status", "--json"]), Duration::from_secs(3)) else {
        return (TailscaleInfo { state: TailscaleState::Disconnected, magic_dns_suffix: None }, Vec::new());
    };
    let state = parse_backend_state(&json).unwrap_or(TailscaleState::Disconnected);
    let connected = state == TailscaleState::Connected;
    let magic_dns_suffix = connected.then(|| parse_magic_dns_suffix(&json)).flatten();
    (TailscaleInfo { state, magic_dns_suffix }, if connected { parse_online_peers(&json) } else { Vec::new() })
}

/// Adresse du tailnet : 100.64.0.0/10 (CGNAT, plage réservée à Tailscale).
pub fn is_tailnet_ip(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && o[1] & 0xC0 == 64
}

/// Pairs en ligne (`Peer[*].Online`) avec leur première IPv4 du tailnet. Au plus 64 ; seule l'adresse est gardée (jamais le nom de machine).
pub fn parse_online_peers(json: &str) -> Vec<Ipv4Addr> {
    let Ok(v) = serde_json::from_str::<Value>(json) else { return Vec::new() };
    let Some(peers) = v.get("Peer").and_then(Value::as_object) else { return Vec::new() };
    peers
        .values()
        .filter(|p| p.get("Online").and_then(Value::as_bool) == Some(true))
        .filter_map(|p| p.get("TailscaleIPs")?.as_array()?.iter().filter_map(|i| i.as_str()?.parse::<Ipv4Addr>().ok()).find(|i| is_tailnet_ip(*i)))
        .take(64)
        .collect()
}

/// Première ligne de `tailscale ip -4`, si c'est bien une adresse du tailnet.
pub fn parse_own_ipv4(out: &str) -> Option<Ipv4Addr> {
    out.lines().next()?.trim().parse::<Ipv4Addr>().ok().filter(|i| is_tailnet_ip(*i))
}

/// IPv4 Tailscale de ce PC (`tailscale ip -4`).
pub fn own_ipv4() -> Option<Ipv4Addr> {
    parse_own_ipv4(&run(Command::new(cli_path()?).args(["ip", "-4"]), Duration::from_secs(3))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_backend_state() {
        assert_eq!(parse_backend_state(r#"{"Version":"1","BackendState":"Running","Peer":{}}"#), Some(TailscaleState::Connected));
        for s in ["Stopped", "NeedsLogin", "NoState", "Starting"] {
            assert_eq!(parse_backend_state(&format!(r#"{{"BackendState":"{s}"}}"#)), Some(TailscaleState::Disconnected));
        }
        assert_eq!(parse_backend_state("nope"), None);
        assert_eq!(parse_backend_state("{}"), None);
    }

    #[test]
    fn magic_dns_suffix_and_host() {
        let j = r#"{"BackendState":"Running","MagicDNSSuffix":"tail1234.ts.net"}"#;
        assert_eq!(parse_magic_dns_suffix(j).as_deref(), Some("tail1234.ts.net"));
        assert_eq!(parse_magic_dns_suffix(r#"{"CurrentTailnet":{"MagicDNSSuffix":"x.ts.net."}}"#).as_deref(), Some("x.ts.net"));
        assert_eq!(parse_magic_dns_suffix(r#"{"MagicDNSSuffix":"a b;rm"}"#), None);
        assert_eq!(parse_magic_dns_suffix("{}"), None);
        assert_eq!(server_host("home-server", Some("x.ts.net")), "home-server.x.ts.net");
        assert_eq!(server_host("home-server", None), "home-server");
    }

    #[test]
    fn peers_and_own_ip() {
        // Fixture inventée : aucune vraie sortie de la CLI.
        let j = r#"{"BackendState":"Running","Self":{"Online":true,"TailscaleIPs":["100.64.0.1"]},"Peer":{
            "nodekey:a":{"Online":true,"TailscaleIPs":["fd7a:115c:a1e0::1","100.101.102.103"]},
            "nodekey:b":{"Online":false,"TailscaleIPs":["100.64.0.9"]},
            "nodekey:c":{"Online":true,"TailscaleIPs":["192.168.1.5"]},
            "nodekey:d":{"Online":true}}}"#;
        assert_eq!(parse_online_peers(j), vec!["100.101.102.103".parse::<Ipv4Addr>().unwrap()]);
        assert!(parse_online_peers("nope").is_empty() && parse_online_peers("{}").is_empty());
        for (ip, ok) in [("100.64.0.1", true), ("100.127.255.254", true), ("100.128.0.1", false), ("100.63.0.1", false), ("192.168.1.2", false), ("0.0.0.0", false)] {
            assert_eq!(is_tailnet_ip(ip.parse().unwrap()), ok, "{ip}");
        }
        assert_eq!(parse_own_ipv4("100.64.0.1
"), Some("100.64.0.1".parse().unwrap()));
        assert_eq!(parse_own_ipv4("192.168.0.4
"), None);
        assert_eq!(parse_own_ipv4(""), None);
    }
}
