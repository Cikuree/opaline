//! `ServerView` : état du serveur d'un pack (ping + diagnostic Tailscale pour un nom court ; ping direct pour un serveur
//! public). L'adresse n'y figure jamais.
use crate::catalog::{PackEntry, ServerInfo};
use crate::ping::{self, ServerStatus};
use crate::tailscale::{self, TailscaleState};
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SamplePlayer {
    pub name: String,
    pub uuid: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Players {
    pub online: u32,
    pub max: u32,
    pub sample: Vec<SamplePlayer>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    pub online: bool,
    pub latency_ms: Option<u64>,
    pub players: Players,
    pub version: Option<String>,
    pub motd_ok: bool,
    pub help: Option<&'static str>,
    /// Vrai si le serveur passe par Tailscale (nom court) ; faux pour un serveur public.
    pub via_tailscale: bool,
}

/// Aide affichée quand le serveur n'est pas utilisable. `ts` : None pour un serveur public (sans Tailscale).
/// `overridden` : adresse de dev, Tailscale non exigé.
pub fn help_for(ts: Option<TailscaleState>, overridden: bool, ping_ok: bool, motd_ok: bool) -> Option<&'static str> {
    if ping_ok {
        return (!motd_ok).then_some("unexpectedServer");
    }
    if overridden {
        return Some("noAccess");
    }
    let Some(ts) = ts else { return Some("serverOffline") };
    Some(match ts {
        TailscaleState::NotInstalled => "tailscaleNotInstalled",
        TailscaleState::Disconnected => "tailscaleDisconnected",
        TailscaleState::Connected => "noAccess",
    })
}

pub fn build(st: Option<&ServerStatus>, motd: &str, ts: Option<TailscaleState>, overridden: bool) -> ServerView {
    let motd_ok = st.is_some_and(|s| ping::motd_contains(s, motd));
    ServerView {
        online: st.is_some(),
        latency_ms: st.and_then(|s| s.latency_ms),
        players: Players {
            online: st.map_or(0, |s| s.online),
            max: st.map_or(0, |s| s.max),
            sample: st.map_or(vec![], |s| {
                s.sample
                    .iter()
                    .zip(s.sample_ids.iter().chain(std::iter::repeat(&String::new())))
                    .map(|(n, u)| SamplePlayer { name: n.clone(), uuid: u.clone() })
                    .collect()
            }),
        },
        version: st.map(|s| s.version.clone()).filter(|v| !v.is_empty()),
        motd_ok,
        help: help_for(ts, overridden, st.is_some(), motd_ok),
        via_tailscale: ts.is_some(),
    }
}

/// État Tailscale pour cette exécution (adresse de dev : Tailscale non exigé).
pub fn tailscale_info() -> tailscale::TailscaleInfo {
    if crate::config::server_overridden() { tailscale::TailscaleInfo { state: TailscaleState::Connected, magic_dns_suffix: None } } else { tailscale::status() }
}

/// Une vérification (3 s au plus par étape) : nom court = Tailscale puis ping ; serveur public = ping direct.
/// `None` : le pack n'a pas de serveur.
pub fn check(e: &PackEntry) -> Option<(ServerView, Option<ServerStatus>)> {
    let srv = e.server.as_ref()?;
    let info = (crate::config::is_short_host(&srv.host)).then(tailscale_info);
    Some(check_with(srv, info.as_ref()))
}

/// `info` : état Tailscale, `None` pour un serveur public.
pub fn check_with(srv: &ServerInfo, info: Option<&tailscale::TailscaleInfo>) -> (ServerView, Option<ServerStatus>) {
    let overridden = crate::config::server_overridden();
    let st = if !overridden && info.is_some_and(|i| i.state != TailscaleState::Connected) {
        None
    } else {
        let suffix = info.and_then(|i| i.magic_dns_suffix.as_deref());
        ping::ping(&crate::config::server_addr(&srv.host, srv.port, suffix), Duration::from_secs(3)).ok()
    };
    (build(st.as_ref(), &srv.motd, info.map(|i| i.state), overridden), st)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(motd: &str) -> ServerStatus {
        ServerStatus {
            version: "26.3".into(),
            protocol: 777,
            online: 2,
            max: 8,
            sample: vec!["A".into(), "B".into()],
            sample_ids: vec!["u1".into()],
            motd: motd.into(),
            latency_ms: Some(12),
        }
    }

    #[test]
    fn help_follows_state() {
        use TailscaleState::*;
        assert_eq!(help_for(Some(NotInstalled), false, false, false), Some("tailscaleNotInstalled"));
        assert_eq!(help_for(Some(Disconnected), false, false, false), Some("tailscaleDisconnected"));
        assert_eq!(help_for(Some(Connected), false, false, false), Some("noAccess"));
        assert_eq!(help_for(Some(NotInstalled), true, false, false), Some("noAccess"));
        assert_eq!(help_for(Some(Connected), false, true, false), Some("unexpectedServer"));
        assert_eq!(help_for(Some(Connected), false, true, true), None);
        // serveur public : jamais de mention de Tailscale
        assert_eq!(help_for(None, false, false, false), Some("serverOffline"));
        assert_eq!(help_for(None, false, true, false), Some("unexpectedServer"));
        assert_eq!(help_for(None, false, true, true), None);
    }

    #[test]
    fn view_maps_status() {
        let v = build(Some(&status("\u{a7}aClassroom Run")), "Classroom Run", Some(TailscaleState::Connected), false);
        assert!(v.online && v.motd_ok && v.help.is_none() && v.via_tailscale);
        assert_eq!((v.players.online, v.players.sample.len(), v.latency_ms), (2, 2, Some(12)));
        assert_eq!(v.players.sample[1].uuid, "");
        let j = serde_json::to_value(&v).unwrap();
        assert!(j.get("latencyMs").is_some() && j.get("motdOk").is_some());
        let off = build(None, "x", Some(TailscaleState::Disconnected), false);
        assert!(!off.online && off.version.is_none() && off.help == Some("tailscaleDisconnected"));
        assert_eq!(build(Some(&status("Autre")), "Classroom Run", Some(TailscaleState::Connected), false).help, Some("unexpectedServer"));
    }
}
