//! Détection locale de l'état de Tailscale. Rien de la sortie de la CLI (noms de machines, IP) n'est loggé ni conservé.
use serde_json::Value;
use std::io::Read;
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
    let Some(cli) = cli_path() else {
        return TailscaleInfo { state: TailscaleState::NotInstalled, magic_dns_suffix: None };
    };
    // CLI présente mais en échec (service arrêté, délai dépassé) : non connecté.
    let Some(json) = run(Command::new(cli).args(["status", "--json"]), Duration::from_secs(3)) else {
        return TailscaleInfo { state: TailscaleState::Disconnected, magic_dns_suffix: None };
    };
    let state = parse_backend_state(&json).unwrap_or(TailscaleState::Disconnected);
    let magic_dns_suffix = (state == TailscaleState::Connected).then(|| parse_magic_dns_suffix(&json)).flatten();
    TailscaleInfo { state, magic_dns_suffix }
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
        assert_eq!(server_host("classroom", Some("x.ts.net")), "classroom.x.ts.net");
        assert_eq!(server_host("classroom", None), "classroom");
    }

    #[test]
    fn messages_follow_state() {
        assert_eq!(message(TailscaleState::NotInstalled, true), Some(MSG_NOT_INSTALLED));
        assert_eq!(message(TailscaleState::Disconnected, false), Some(MSG_DISCONNECTED));
        assert_eq!(message(TailscaleState::Connected, false), Some(MSG_UNREACHABLE));
        assert_eq!(message(TailscaleState::Connected, true), None);
    }
}
