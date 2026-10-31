//! Identité sans authentification (mode officiel) : pseudo Minecraft -> UUID (API publique Mojang), puis skin du sessionserver.
//! Aucun jeton, aucun fichier du Launcher officiel : seulement des API publiques en lecture.
use crate::head;
use crate::skins::fetch_texture;
use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::Value;

pub const PROFILE_API: &str = "https://api.mojang.com";

/// Pseudo Java valide : 3 à 16 caractères `[A-Za-z0-9_]`. Vérifié avant tout appel réseau.
pub fn validate_username(name: &str) -> Result<&str> {
    let n = name.trim();
    if !(3..=16).contains(&n.len()) || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("Pseudo invalide : 3 à 16 caractères, lettres, chiffres ou _");
    }
    Ok(n)
}

/// Réponse de `users/profiles/minecraft/<pseudo>` : 200 -> (pseudo exact, UUID) ; 204 et 404 -> `None` (pseudo inconnu).
pub fn parse_lookup(status: u16, body: &Value) -> Result<Option<(String, String)>> {
    match status {
        200 => {
            let name = body.get("name").and_then(Value::as_str).context("réponse Mojang sans pseudo")?;
            let id = head::normalize_uuid(body.get("id").and_then(Value::as_str).context("réponse Mojang sans UUID")?)?;
            Ok(Some((name.to_string(), id)))
        }
        204 | 404 => Ok(None),
        s => bail!("Mojang indisponible (HTTP {s})"),
    }
}

/// Résout un pseudo en `(pseudo exact, UUID)`.
pub fn lookup(client: &Client, api: &str, name: &str) -> Result<(String, String)> {
    let n = validate_username(name)?;
    let r = client
        .get(format!("{}/users/profiles/minecraft/{n}", api.trim_end_matches('/')))
        .send()
        .map_err(|_| anyhow!("Mojang injoignable : vérifie ton réseau"))?;
    let status = r.status().as_u16();
    let body: Value = if status == 200 { r.json().map_err(|_| anyhow!("réponse Mojang illisible"))? } else { Value::Null };
    parse_lookup(status, &body)?.with_context(|| format!("Aucun joueur Minecraft nommé « {n} »"))
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkinOf {
    pub data_url: String,
    pub model: String,
}

/// Skin complet (PNG en data URL) et modèle de bras, d'après le sessionserver public.
pub fn skin_of(client: &Client, session_server: &str, uuid: &str) -> Result<SkinOf> {
    let id = head::normalize_uuid(uuid)?;
    let r = client
        .get(format!("{}/session/minecraft/profile/{id}", session_server.trim_end_matches('/')))
        .send()
        .map_err(|_| anyhow!("profil indisponible (réseau)"))?;
    if !r.status().is_success() {
        bail!("profil indisponible (HTTP {})", r.status().as_u16());
    }
    let profile: Value = r.json().map_err(|_| anyhow!("profil illisible"))?;
    let png = fetch_texture(client, &head::skin_url_from_profile(&profile)?)?;
    Ok(SkinOf { data_url: head::data_url(&png), model: head::skin_model_from_profile(&profile).into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn username_validation() {
        for ok in ["Bob", "a_b_c_d_e_f_g_h_", "  Steve_01 "] {
            assert!(validate_username(ok).is_ok(), "{ok}");
        }
        for bad in ["", "ab", "a_b_c_d_e_f_g_h_i", "a b c", "é_é_é", "../x", "a/b", "Bob?x=1"] {
            assert!(validate_username(bad).is_err(), "{bad}");
        }
        assert_eq!(validate_username(" Bob ").unwrap(), "Bob");
    }

    #[test]
    fn lookup_response_parsing() {
        let ok = json!({"id": "B50AD385829D3141A2167E7D7539BA7F", "name": "Bob"});
        assert_eq!(parse_lookup(200, &ok).unwrap(), Some(("Bob".into(), "b50ad385829d3141a2167e7d7539ba7f".into())));
        assert_eq!(parse_lookup(204, &Value::Null).unwrap(), None);
        assert_eq!(parse_lookup(404, &json!({"errorMessage": "x"})).unwrap(), None);
        assert!(parse_lookup(429, &Value::Null).is_err());
        assert!(parse_lookup(200, &json!({"name": "Bob"})).is_err());
        assert!(parse_lookup(200, &json!({"name": "Bob", "id": "pasunuuid"})).is_err());
    }

    #[test]
    fn skin_model_from_metadata() {
        use base64::Engine;
        let mk = |meta: &str| {
            let t = base64::engine::general_purpose::STANDARD
                .encode(format!(r#"{{"textures":{{"SKIN":{{"url":"https://textures.minecraft.net/texture/x"{meta}}}}}}}"#));
            json!({"properties": [{"name": "textures", "value": t}]})
        };
        assert_eq!(head::skin_model_from_profile(&mk(r#","metadata":{"model":"slim"}"#)), "slim");
        assert_eq!(head::skin_model_from_profile(&mk("")), "classic");
        assert_eq!(head::skin_model_from_profile(&json!({})), "classic");
    }
}
