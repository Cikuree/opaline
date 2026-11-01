//! Joueurs vus en ligne sur les serveurs des packs (ping / stats), mémorisés localement.
//! Pas de service de présence : « en ligne » = vu au dernier relevé.
use crate::config::Dirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_KNOWN: usize = 500;

#[derive(Serialize, Deserialize, Default, Debug)]
struct Store {
    /// uuid (avec tirets, minuscules) -> (nom, dernière visite en secondes Unix)
    players: BTreeMap<String, (String, u64)>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub name: String,
    pub uuid: String,
    pub online: bool,
    pub in_game: bool,
    pub last_seen: Option<String>,
}

/// Pseudo Minecraft : 1 à 16 caractères `[A-Za-z0-9_]`.
pub fn is_mc_name(n: &str) -> bool {
    (1..=16).contains(&n.len()) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// UUID en forme à tirets, minuscules ; `None` si invalide.
pub fn dashed(u: &str) -> Option<String> {
    let s = crate::head::normalize_uuid(u).ok()?;
    Some(format!("{}-{}-{}-{}-{}", &s[..8], &s[8..12], &s[12..16], &s[16..20], &s[20..]))
}

/// Secondes Unix -> `AAAA-MM-JJThh:mm:ssZ` (algorithme civil de H. Hinnant).
pub fn iso8601(secs: u64) -> String {
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem / 60 % 60, rem % 60)
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn load(dirs: &Dirs) -> Store {
    std::fs::read(dirs.cache().join("friends.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Enregistre le relevé (`seen` = nom, uuid des joueurs vus en ligne) et renvoie la liste complète,
/// triée : en ligne d'abord, puis par nom. `in_game` = vu sur un serveur de pack à ce relevé.
pub fn record(dirs: &Dirs, seen: &[(String, String)], now: u64) -> Vec<Friend> {
    let mut st = load(dirs);
    let mut online = std::collections::BTreeSet::new();
    for (name, uuid) in seen {
        let (Some(id), true) = (dashed(uuid), is_mc_name(name)) else {
            continue;
        };
        st.players.insert(id.clone(), (name.clone(), now));
        online.insert(id);
    }
    // Garde les plus récents si le cache grossit.
    if st.players.len() > MAX_KNOWN {
        let mut v: Vec<_> = st.players.iter().map(|(k, (_, t))| (*t, k.clone())).collect();
        v.sort();
        for (_, k) in v.into_iter().take(st.players.len() - MAX_KNOWN) {
            st.players.remove(&k);
        }
    }
    if std::fs::create_dir_all(dirs.cache()).is_ok() {
        let _ = serde_json::to_vec(&st).map(|b| std::fs::write(dirs.cache().join("friends.json"), b));
    }
    let mut out: Vec<Friend> = st
        .players
        .into_iter()
        .map(|(uuid, (name, t))| {
            let on = online.contains(&uuid);
            Friend { name, uuid, online: on, in_game: on, last_seen: Some(iso8601(t)) }
        })
        .collect();
    out.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

// ---------- Liste d'amis choisie par l'utilisateur ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(1_709_164_800 + 3661), "2024-02-29T01:01:01Z");
    }

    #[test]
    fn records_orders_and_remembers() {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-friends-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d.root);
        let u = |n: u8| format!("{n:032x}");
        let first = record(&d, &[("Zed".into(), u(1)), ("alice".into(), u(2)), ("bad name!!!!!!!!!!!!".into(), u(3)), ("X".into(), "nope".into())], 100);
        assert_eq!(first.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["alice", "Zed"]);
        let second = record(&d, &[("Zed".into(), u(1))], 200);
        assert_eq!(second.iter().map(|f| (f.name.as_str(), f.online)).collect::<Vec<_>>(), [("Zed", true), ("alice", false)]);
        assert_eq!(second[1].last_seen.as_deref(), Some("1970-01-01T00:01:40Z"));
        assert_eq!(second[0].uuid, "00000000-0000-0000-0000-000000000001");
        std::fs::remove_dir_all(&d.root).unwrap();
    }
}
