//! Session de jeu. Étape 2 : mode hors ligne uniquement.
//! ETAPE 3 (à venir) : auth Microsoft (device code -> XBL -> XSTS -> Minecraft), tokens dans le
//! trousseau système (crate `keyring`) ; elle devra produire une `Session` identique.
use md5::{Digest, Md5};
use std::fmt;

#[derive(Clone)]
pub struct Session {
    pub username: String,
    pub uuid: String,
    /// "legacy" hors ligne ; "msa" avec l'auth Microsoft (étape 3).
    pub user_type: &'static str,
    access_token: String,
}

impl Session {
    /// Session hors ligne (dev) : UUID déterministe comme le serveur vanilla hors ligne, token factice.
    pub fn offline(nickname: &str) -> Session {
        Session { username: nickname.to_string(), uuid: offline_uuid(nickname), user_type: "legacy", access_token: "0".into() }
    }

    pub fn access_token(&self) -> &str {
        &self.access_token
    }
}

// Pas de token dans les logs : Debug le masque.
impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session").field("username", &self.username).field("uuid", &self.uuid).field("access_token", &"<masqué>").finish()
    }
}

/// UUID v3 de `"OfflinePlayer:" + nom` (équivalent de `UUID.nameUUIDFromBytes` de Java).
pub fn offline_uuid(name: &str) -> String {
    let d: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    uuid::Builder::from_md5_bytes(d).into_uuid().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        // Valeur connue du serveur vanilla hors ligne pour le pseudo « Notch ».
        assert_eq!(offline_uuid("Notch"), "b50ad385-829d-3141-a216-7e7d7539ba7f");
    }

    #[test]
    fn debug_hides_token() {
        let s = Session::offline("A");
        assert!(!format!("{s:?}").contains("\"0\""));
        assert!(format!("{s:?}").contains("masqué"));
    }
}
