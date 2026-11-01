//! Statut Discord (Rich Presence) : « Joue à <Pack> » avec l'heure de début, icône générique « opaline », aucun bouton.
//! Identifiant d'application injecté à la compilation (`DISCORD_APP_ID`) ; absent ou vide : fonctionnalité désactivée.
//! Tout échec (Discord fermé, tube illisible) est silencieux : le prochain changement d'état retente la connexion.
use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use std::sync::mpsc::{Receiver, Sender, channel};

/// Texte exact attendu pour l'image à téléverser dans le portail Discord (voir le README de l'application).
const LARGE_IMAGE: &str = "opaline";

pub type Message = Option<(String, u64)>;

/// Identifiant d'application non vide, ou `None`.
pub fn app_id() -> Option<&'static str> {
    clean(option_env!("DISCORD_APP_ID"))
}

fn clean(id: Option<&'static str>) -> Option<&'static str> {
    id.map(str::trim).filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
}

/// Démarre le fil de présence ; `None` si l'application est compilée sans identifiant Discord.
pub fn start() -> Option<Sender<Message>> {
    let id = app_id()?;
    let (tx, rx) = channel();
    std::thread::spawn(move || run(id, rx));
    Some(tx)
}

fn run(id: &str, rx: Receiver<Message>) {
    let mut client: Option<DiscordIpcClient> = None;
    for msg in rx {
        match msg {
            Some((pack, since)) => {
                if client.is_none() {
                    let mut c = DiscordIpcClient::new(id);
                    client = c.connect().ok().map(|()| c);
                }
                let ok = client.as_mut().is_some_and(|c| {
                    c.set_activity(
                        Activity::new()
                            .details(format!("Joue à {pack}"))
                            .timestamps(Timestamps::new().start(i64::try_from(since).unwrap_or(0)))
                            .assets(Assets::new().large_image(LARGE_IMAGE).large_text("Opaline")),
                    )
                    .is_ok()
                });
                if !ok {
                    client = None; // connexion perdue : reconnexion au prochain message
                }
            }
            None => {
                if let Some(mut c) = client.take() {
                    let _ = c.clear_activity();
                    let _ = c.close();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_invalid_app_id_disables_presence() {
        assert_eq!(clean(None), None);
        assert_eq!(clean(Some("")), None);
        assert_eq!(clean(Some("  ")), None);
        assert_eq!(clean(Some("abc")), None);
        assert_eq!(clean(Some(" 123456789012345678 ")), Some("123456789012345678"));
    }

    #[test]
    fn build_without_id_has_no_thread() {
        if app_id().is_none() {
            assert!(start().is_none());
        }
    }
}
