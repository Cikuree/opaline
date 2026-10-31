//! Cas 5 : `sanitize_error` (chemins personnels), `log_tail` expurgé (jeton, adresse serveur),
//! erreurs des commandes sans adresse de serveur ni jeton.
#![allow(dead_code)]
mod opaline_common;
mod support;

use directories::BaseDirs;
use opaline_common::*;
use opaline_core::config::Dirs;
use opaline_core::service;
use std::fs;
use support::{Mode, Server, dead_base};

const TOKEN: &str = "eyJSECRETTOKEN.abc.def";
const SERVER: &str = "secret.tail.ts.net:25565";

fn root_under(t: &Tmp) -> Dirs {
    Dirs::new(t.0.join("Opaline"))
}

#[test]
fn sanitize_replaces_data_dir_and_home_in_every_slash_style() {
    let t = Tmp::new("san");
    let root = root_under(&t);
    let inst = root.root.join("instances").join("alpha").join("mods");
    let p = inst.display().to_string();
    let fwd = p.replace('\\', "/");
    let back = p.replace('/', "\\");
    for form in [&p, &fwd, &back] {
        let m = service::sanitize_error(&format!("écriture impossible dans {form}"), &root);
        assert!(m.contains("<Opaline>") && !m.contains(&root.root.display().to_string()), "{m}");
        assert!(!m.contains("instances") || m.contains("<Opaline>"), "{m}");
    }
    if let Some(home) = BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
        let m = service::sanitize_error(&format!("lecture de {}", home.join("Documents").join("x.txt").display()), &root);
        assert!(!m.contains(&home.display().to_string()), "dossier utilisateur non masqué : {m}");
        assert!(m.contains('~'), "{m}");
    }
    // Un message sans chemin est inchangé.
    assert_eq!(service::sanitize_error("HTTP 503 pour le pack", &root), "HTTP 503 pour le pack");
}

#[test]
fn log_tail_hides_token_server_host_and_personal_paths() {
    let t = Tmp::new("log");
    let root = root_under(&t);
    let log = t.0.join("g.log");
    let content = format!(
        "[Render] Connecting to secret.tail.ts.net, 25565\n\
         auth {TOKEN} ok\n\
         joined {SERVER} as {TOKEN}\n\
         chargement {}\n\
         fin du jeu\n",
        root.root.join("instances").join("alpha").display()
    );
    fs::write(&log, content).unwrap();
    let tail = service::log_tail(&log, TOKEN, SERVER, &root, 10).unwrap();
    assert!(!tail.contains(TOKEN), "jeton présent : {tail}");
    assert!(!tail.contains("secret.tail"), "hôte présent : {tail}");
    assert!(!tail.contains(&root.root.display().to_string()), "chemin personnel présent : {tail}");
    assert!(tail.contains("<masqué>") && tail.contains("<serveur>") && tail.contains("<Opaline>"), "{tail}");
    assert!(tail.ends_with("fin du jeu"));
}

#[test]
fn log_tail_keeps_only_requested_last_lines() {
    let t = Tmp::new("log-n");
    let root = root_under(&t);
    let log = t.0.join("g.log");
    let lines: Vec<String> = (1..=50).map(|i| format!("ligne {i}")).collect();
    fs::write(&log, lines.join("\n")).unwrap();
    let tail = service::log_tail(&log, TOKEN, SERVER, &root, 3).unwrap();
    assert_eq!(tail, "ligne 48\nligne 49\nligne 50");
    assert!(service::log_tail(&t.0.join("absent.log"), TOKEN, SERVER, &root, 3).is_none());
}

#[test]
fn log_tail_without_server_or_token_does_not_mangle_text() {
    let t = Tmp::new("log-plain");
    let root = root_under(&t);
    let log = t.0.join("g.log");
    fs::write(&log, "Done (3.2s)!\nTexte ordinaire é ü").unwrap();
    let tail = service::log_tail(&log, "", "", &root, 5).unwrap();
    assert_eq!(tail, "Done (3.2s)!\nTexte ordinaire é ü");
}

#[test]
fn run_stats_errors_do_not_reveal_server_address() {
    let _g = env_lock();
    let t = Tmp::new("err-stats");
    let root = Dirs::new(t.0.join("root"));
    let dead = dead_base();
    let dead_port = dead.rsplit(':').next().unwrap().to_string();
    let f = t.0.join("packs.json");
    fs::write(
        &f,
        format!(
            r#"{{"packs":[{{"id":"classroom-run","name":"CR","mc":"26.3","loader":"fabric","pack":"https://github.com/x/p.zip","packHash":"abcdef0123456789","widgets":["classroomRun","server"],"server":{{"host":"classroom","port":25565,"motd":"Classroom Run","statsPort":{dead_port}}}}}]}}"#
        ),
    )
    .unwrap();
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    set_env("CLASSROOM_SERVER", "127.0.0.1:25565");

    let err = service::run_stats(&root, "classroom-run").unwrap_err().to_string();
    assert!(!err.contains("127.0.0.1") && !err.contains(&dead_port), "adresse dans l'erreur : {err}");
    assert!(err.contains("injoignable"), "{err}");

    let srv = Server::start();
    srv.put_mode("/stats", "", Mode::Status(500));
    let port = srv.base.rsplit(':').next().unwrap().to_string();
    fs::write(
        &f,
        format!(
            r#"{{"packs":[{{"id":"classroom-run","name":"CR","mc":"26.3","loader":"fabric","pack":"https://github.com/x/p.zip","packHash":"abcdef0123456789","widgets":["classroomRun","server"],"server":{{"host":"classroom","port":25565,"motd":"Classroom Run","statsPort":{port}}}}}]}}"#
        ),
    )
    .unwrap();
    let err = service::run_stats(&root, "classroom-run").unwrap_err().to_string();
    assert!(err.contains("HTTP 500") && !err.contains("127.0.0.1") && !err.contains(&port), "{err}");
    assert!(!err.contains(TOKEN));

    unset_env("CLASSROOM_SERVER");
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn server_status_view_never_contains_the_address() {
    let _g = env_lock();
    let t = Tmp::new("err-status");
    let root = Dirs::new(t.0.join("root"));
    let dead = dead_base();
    let f = t.0.join("packs.json");
    fs::write(
        &f,
        r#"{"packs":[{"id":"classroom-run","name":"CR","mc":"26.3","loader":"fabric","pack":"https://github.com/x/p.zip","packHash":"abcdef0123456789","server":{"host":"classroom","port":25565,"motd":"Classroom Run"}}]}"#,
    )
    .unwrap();
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    set_env("CLASSROOM_SERVER", dead.trim_start_matches("http://"));

    let v = service::server_status(&root, "classroom-run").unwrap().expect("pack avec serveur");
    assert!(!v.online);
    assert_eq!(v.help, Some("noAccess"));
    let json = serde_json::to_string(&v).unwrap();
    assert!(!json.contains("127.0.0.1") && !json.contains(&dead.rsplit(':').next().unwrap().to_string()), "{json}");

    unset_env("CLASSROOM_SERVER");
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn pack_without_server_has_no_status_stats_or_history() {
    let _g = env_lock();
    let t = Tmp::new("no-server");
    let root = Dirs::new(t.0.join("root"));
    let f = t.0.join("packs.json");
    fs::write(
        &f,
        r#"{"packs":[{"id":"solo","name":"Solo","mc":"26.3","loader":"fabric","pack":"https://github.com/x/p.zip","packHash":"abcdef0123456789","widgets":["classroomRun","server"]}]}"#,
    )
    .unwrap();
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    assert!(service::server_status(&root, "solo").unwrap().is_none());
    assert!(service::run_stats(&root, "solo").unwrap().is_none());
    assert!(service::run_history(&root, "solo", None).unwrap().is_none());
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn public_server_offline_gives_plain_help_without_tailscale() {
    let _g = env_lock();
    let t = Tmp::new("public-off");
    let root = Dirs::new(t.0.join("root"));
    let f = t.0.join("packs.json");
    fs::write(
        &f,
        r#"{"packs":[{"id":"pub","name":"Pub","mc":"26.3","loader":"fabric","pack":"https://github.com/x/p.zip","packHash":"abcdef0123456789","server":{"host":"127.0.0.1","port":1,"motd":"Pub"}}]}"#,
    )
    .unwrap();
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    let v = service::server_status(&root, "pub").unwrap().unwrap();
    assert!(!v.online && !v.via_tailscale);
    assert_eq!(v.help, Some("serverOffline"));
    unset_env("OPALINE_PACKS_URL");
}

#[test]
fn errors_from_pack_install_mention_no_token_or_path() {
    let _g = env_lock();
    let t = Tmp::new("err-inst");
    let root = Dirs::new(t.0.join("Opaline"));
    let f = t.0.join("packs.json");
    fs::write(&f, catalog_json(&[catalog_entry("neo", "neoforge", "https://github.com/x/n.zip", "abcdef0123456789")])).unwrap();
    set_env("OPALINE_PACKS_URL", f.to_str().unwrap());
    let e = service::install_pack(&root, "neo", &std::sync::Arc::default(), None).unwrap_err().to_string();
    assert!(!e.contains(TOKEN) && !e.contains(&t.0.display().to_string()), "{e}");
    let clean = service::sanitize_error(&e, &root);
    assert_eq!(clean, e, "aucun chemin à masquer dans ce message");
    unset_env("OPALINE_PACKS_URL");
}
