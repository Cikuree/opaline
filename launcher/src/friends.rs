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

const MAX_FRIENDS: usize = 200;

#[derive(Serialize, Deserialize, Default, Debug)]
struct ListFile {
    /// uuid (avec tirets, minuscules) -> pseudo
    friends: BTreeMap<String, String>,
}

fn list_path(dirs: &Dirs) -> std::path::PathBuf {
    dirs.root.join("friends_list.json")
}

/// Amis ajoutés à la main (uuid -> pseudo) ; vide si le fichier manque ou est illisible.
pub fn list_load(dirs: &Dirs) -> BTreeMap<String, String> {
    std::fs::read(list_path(dirs)).ok().and_then(|b| serde_json::from_slice::<ListFile>(&b).ok()).map(|f| f.friends).unwrap_or_default()
}

/// Écriture atomique : fichier temporaire puis renommage.
fn list_save(dirs: &Dirs, friends: BTreeMap<String, String>) -> anyhow::Result<()> {
    std::fs::create_dir_all(&dirs.root)?;
    let (path, tmp) = (list_path(dirs), dirs.root.join("friends_list.json.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&ListFile { friends })?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Ajoute (ou renomme) un ami ; renvoie son UUID normalisé. `me` : UUID de l'utilisateur, refusé.
pub fn list_add(dirs: &Dirs, name: &str, uuid: &str, me: Option<&str>) -> anyhow::Result<String> {
    let id = dashed(uuid).ok_or_else(|| anyhow::anyhow!("UUID invalide"))?;
    anyhow::ensure!(is_mc_name(name), "Pseudo invalide");
    anyhow::ensure!(me.and_then(dashed).as_deref() != Some(id.as_str()), "C’est ton propre pseudo");
    let mut l = list_load(dirs);
    anyhow::ensure!(l.contains_key(&id) || l.len() < MAX_FRIENDS, "Liste d’amis pleine ({MAX_FRIENDS} maximum)");
    l.insert(id.clone(), name.to_string());
    list_save(dirs, l)?;
    Ok(id)
}

pub fn list_remove(dirs: &Dirs, uuid: &str) -> anyhow::Result<()> {
    let id = dashed(uuid).ok_or_else(|| anyhow::anyhow!("UUID invalide"))?;
    let mut l = list_load(dirs);
    if l.remove(&id).is_some() {
        list_save(dirs, l)?;
    }
    Ok(())
}

/// Ligne de la colonne d'amis : joueurs vus sur les serveurs, amis ajoutés et pairs Opaline du tailnet, fusionnés par UUID.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FriendView {
    pub name: String,
    pub uuid: String,
    pub online: bool,
    pub in_game: bool,
    pub last_seen: Option<String>,
    /// Dans la liste d'amis (faux : joueur vu sur un serveur, ou pair Tailscale à ajouter).
    pub friend: bool,
    /// Une présence Opaline a répondu sur le tailnet.
    pub p2p: bool,
    pub pack_id: Option<String>,
    pub pack_version: Option<String>,
}

fn view(name: &str, uuid: &str, friend: bool) -> FriendView {
    FriendView { name: name.into(), uuid: uuid.into(), online: false, in_game: false, last_seen: None, friend, p2p: false, pack_id: None, pack_version: None }
}

/// Fusion par UUID : `seen` (joueurs vus), `list` (amis ajoutés), `presences` (réponses P2P). `me` est exclu des pairs.
pub fn merge(list: &BTreeMap<String, String>, seen: Vec<Friend>, presences: &[crate::p2p::Presence], me: Option<&str>) -> Vec<FriendView> {
    let me = me.and_then(dashed);
    let mut by: BTreeMap<String, FriendView> = BTreeMap::new();
    for f in seen {
        let id = dashed(&f.uuid).unwrap_or_else(|| f.uuid.clone());
        by.insert(id.clone(), FriendView { online: f.online, in_game: f.in_game, last_seen: f.last_seen, ..view(&f.name, &id, list.contains_key(&id)) });
    }
    for (id, name) in list {
        by.entry(id.clone()).or_insert_with(|| view(name, id, true));
    }
    for p in presences.iter().filter(|p| Some(&p.uuid) != me.as_ref()) {
        let v = by.entry(p.uuid.clone()).or_insert_with(|| view(&p.name, &p.uuid, false));
        v.p2p = true;
        v.online = true;
        v.in_game |= p.in_game;
        if p.in_game {
            (v.pack_id, v.pack_version) = (p.pack_id.clone(), p.pack_version.clone());
        }
    }
    let mut out: Vec<FriendView> = by.into_values().collect();
    out.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> Dirs {
        let d = Dirs::new(std::env::temp_dir().join(format!("opaline-fl-{tag}-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d.root);
        d
    }

    #[test]
    fn list_persists_dedups_and_removes() {
        let d = tmp("list");
        let u = |n: u8| format!("{n:032x}");
        assert!(list_load(&d).is_empty());
        let id = list_add(&d, "Alice", &u(1), None).unwrap();
        assert_eq!(id, "00000000-0000-0000-0000-000000000001");
        list_add(&d, "Alice2", &id.to_uppercase(), None).unwrap(); // même UUID : renommé, pas de doublon
        assert_eq!(list_load(&d).len(), 1);
        assert_eq!(list_load(&d)[&id], "Alice2");
        assert!(list_add(&d, "Moi", &u(2), Some(&u(2))).is_err());
        assert!(list_add(&d, "bad name", &u(3), None).is_err() && list_add(&d, "Bob", "zz", None).is_err());
        std::fs::write(list_path(&d), b"{corrompu").unwrap();
        assert!(list_load(&d).is_empty());
        list_add(&d, "Bob", &u(3), None).unwrap();
        list_remove(&d, &u(3)).unwrap();
        assert!(list_load(&d).is_empty() && !d.root.join("friends_list.json.tmp").exists());
        std::fs::remove_dir_all(&d.root).unwrap();
    }

    #[test]
    fn merge_by_uuid_without_duplicates() {
        use crate::p2p::Presence;
        let u = |n: u8| format!("00000000-0000-0000-0000-{n:012x}");
        let seen = vec![
            Friend { name: "Zed".into(), uuid: u(1), online: true, in_game: true, last_seen: Some("t".into()) },
            Friend { name: "Old".into(), uuid: u(2), online: false, in_game: false, last_seen: Some("t".into()) },
        ];
        let list: BTreeMap<String, String> = [(u(1), "Zed".to_string()), (u(3), "Cara".to_string())].into();
        let pres = |n: u8, name: &str, in_game: bool| Presence { app: "opaline".into(), v: 1, uuid: u(n), name: name.into(), in_game, pack_id: in_game.then(|| "survie".to_string()), pack_version: in_game.then(|| "a1".to_string()), app_version: "0.3.0".into() };
        let out = merge(&list, seen, &[pres(3, "Cara", true), pres(4, "Dan", false), pres(1, "Zed", false), pres(9, "Moi", false)], Some(&u(9)));
        let row = |n: &str| out.iter().find(|f| f.name == n).unwrap();
        assert_eq!(out.len(), 4, "un seul exemplaire par UUID, soi-même exclu");
        assert!(row("Cara").friend && row("Cara").in_game && row("Cara").p2p && row("Cara").pack_id.as_deref() == Some("survie"));
        assert!(!row("Dan").friend && row("Dan").p2p && row("Dan").online);
        assert!(row("Zed").friend && row("Zed").in_game && row("Zed").last_seen.is_some());
        assert!(!row("Old").friend && !row("Old").p2p && !row("Old").online);
        assert_eq!(out.last().unwrap().name, "Old");
    }

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
