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

pub const MSG_NOT_INSTALLED: &str = "Tailscale n'est pas installé sur ce PC.";
pub const MSG_DISCONNECTED: &str = "Tailscale n'est pas connecté : ouvre Tailscale et connecte-toi.";
pub const MSG_UNREACHABLE: &str = "Serveur injoignable : demande l'accès à l'hôte ou attends qu'il démarre le serveur.";

/// Message à afficher, ou None si tout va bien. `server_reachable` : résultat du dernier ping.
pub fn message(state: TailscaleState, server_reachable: bool) -> Option<&'static str> {
    match state {
        TailscaleState::NotInstalled => Some(MSG_NOT_INSTALLED),
        TailscaleState::Disconnected => Some(MSG_DISCONNECTED),
        TailscaleState::Connected if !server_reachable => Some(MSG_UNREACHABLE),
        TailscaleState::Connected => None,
    }
}

/// `BackendState` de `tailscale status --json` (champ de `ipnstate.Status`, valeur « Running » = connecté).
/// None si le JSON est invalide.
pub fn parse_backend_state(json: &str) -> Option<TailscaleState> {
    let v: Value = serde_json::from_str(json).ok()?;
    let s = v.get("BackendState")?.as_str()?;
    Some(if s == "Running" { TailscaleState::Connected } else { TailscaleState::Disconnected })
}

/// 100.64.0.0/10 (CGNAT, plage de Tailscale).
pub fn is_tailscale_ip(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (64..=127).contains(&o[1])
}

/// Repli : une adresse de la plage Tailscale apparaît dans la sortie de `ipconfig` / `ip addr`.
pub fn has_tailscale_addr(text: &str) -> bool {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter_map(|t| t.parse::<Ipv4Addr>().ok()).any(is_tailscale_ip)
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

fn local_addrs_text() -> String {
    let mut c = if cfg!(windows) { Command::new("ipconfig") } else {
        let mut c = Command::new("ip");
        c.args(["-o", "-4", "addr"]);
        c
    };
    run(&mut c, Duration::from_secs(3)).unwrap_or_default()
}

pub fn tailscale_state() -> TailscaleState {
    let Some(cli) = cli_path() else {
        return if has_tailscale_addr(&local_addrs_text()) { TailscaleState::Connected } else { TailscaleState::NotInstalled };
    };
    if let Some(st) = run(Command::new(cli).args(["status", "--json"]), Duration::from_secs(3)).and_then(|j| parse_backend_state(&j)) {
        return st;
    }
    if has_tailscale_addr(&local_addrs_text()) { TailscaleState::Connected } else { TailscaleState::Disconnected }
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
    fn detects_cgnat_range() {
        assert!(has_tailscale_addr("Adresse IPv4. . . : 100.101.2.3\n"));
        assert!(has_tailscale_addr("2: tailscale0 inet 100.64.0.1/32 scope global"));
        assert!(!has_tailscale_addr("inet 192.168.1.5 inet 100.63.0.1 inet 100.128.0.1 inet 10.100.64.1"));
    }

    #[test]
    fn messages_follow_state() {
        assert_eq!(message(TailscaleState::NotInstalled, true), Some(MSG_NOT_INSTALLED));
        assert_eq!(message(TailscaleState::Disconnected, false), Some(MSG_DISCONNECTED));
        assert_eq!(message(TailscaleState::Connected, false), Some(MSG_UNREACHABLE));
        assert_eq!(message(TailscaleState::Connected, true), None);
    }
}
