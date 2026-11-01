//! Machine à états de l'intégration au bureau pendant une partie lancée en direct : zone de notification et statut Discord.
//! Pure (aucune interface, aucun réseau) : le backend Tauri exécute les `Action` renvoyées.

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Action {
    /// Cacher la fenêtre dans la zone de notification.
    Hide,
    /// Réafficher la fenêtre.
    Show,
    /// Statut Discord : `Some((nom du pack, début en secondes Unix))`, ou `None` pour l'effacer.
    Presence(Option<(String, u64)>),
}

/// Réglages lus à chaque transition (modifiables en cours de partie).
#[derive(Clone, Copy)]
pub struct Prefs {
    pub tray: bool,
    pub discord: bool,
}

#[derive(Default)]
pub struct Shell {
    /// Parties en cours : (id du pack, nom, début).
    running: Vec<(String, String, u64)>,
    hidden: bool,
}

impl Shell {
    fn presence(&self, p: Prefs) -> Option<Action> {
        // Plusieurs parties : la plus récente est affichée.
        match self.running.last() {
            Some((_, name, since)) => p.discord.then(|| Action::Presence(Some((name.clone(), *since)))),
            None => Some(Action::Presence(None)),
        }
    }

    pub fn game_started(&mut self, id: &str, name: &str, since: u64, p: Prefs) -> Vec<Action> {
        self.running.retain(|r| r.0 != id);
        self.running.push((id.to_string(), name.to_string(), since));
        let mut out = Vec::new();
        if p.tray && !self.hidden {
            self.hidden = true;
            out.push(Action::Hide);
        }
        out.extend(self.presence(p));
        out
    }

    /// La fenêtre est toujours réaffichée à la fin de la dernière partie, même si le réglage a été coupé entre-temps.
    pub fn game_exited(&mut self, id: &str, p: Prefs) -> Vec<Action> {
        self.running.retain(|r| r.0 != id);
        let mut out = Vec::new();
        if self.running.is_empty() && self.hidden {
            self.hidden = false;
            out.push(Action::Show);
        }
        out.extend(self.presence(p));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: Prefs = Prefs { tray: true, discord: true };

    #[test]
    fn hides_and_shows_with_presence() {
        let mut s = Shell::default();
        assert_eq!(s.game_started("a", "Pack A", 100, ON), [Action::Hide, Action::Presence(Some(("Pack A".into(), 100)))]);
        assert_eq!(s.game_exited("a", ON), [Action::Show, Action::Presence(None)]);
        assert_eq!(s.game_exited("a", ON), [Action::Presence(None)]); // sortie en double : rien d'autre
    }

    #[test]
    fn settings_off_do_nothing() {
        let mut s = Shell::default();
        assert!(s.game_started("a", "A", 1, Prefs { tray: false, discord: false }).is_empty());
        assert_eq!(s.game_exited("a", Prefs { tray: false, discord: false }), [Action::Presence(None)]);
    }

    #[test]
    fn two_games_keep_hidden_until_the_last_exit() {
        let mut s = Shell::default();
        s.game_started("a", "A", 1, ON);
        assert_eq!(s.game_started("b", "B", 2, ON), [Action::Presence(Some(("B".into(), 2)))]);
        assert_eq!(s.game_exited("b", ON), [Action::Presence(Some(("A".into(), 1)))]);
        assert_eq!(s.game_exited("a", ON), [Action::Show, Action::Presence(None)]);
    }

    #[test]
    fn tray_turned_off_mid_game_still_shows_again() {
        let mut s = Shell::default();
        s.game_started("a", "A", 1, ON);
        assert_eq!(s.game_exited("a", Prefs { tray: false, discord: false }), [Action::Show, Action::Presence(None)]);
    }
}
