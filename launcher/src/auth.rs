//! Session de jeu et authentification Microsoft (navigateur ou device code -> XBL -> XSTS -> Minecraft).
//!
//! Sécurité : le refresh token Microsoft vit uniquement dans le trousseau système (crate `keyring`) ;
//! les access tokens restent en mémoire. Aucun type porteur de token n'implémente `Debug`, et les messages
//! d'erreur ne contiennent jamais de corps de réponse (seulement le statut HTTP et des codes d'erreur).
//! Les réponses sont lues comme `serde_json::Value` pour que les erreurs de parsing ne citent pas de valeur.
use anyhow::{Context, Result, anyhow, bail};
use md5::{Digest, Md5};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// ID client de l'app Azure (public, non secret), fixé à la compilation. Vide = connexion non configurée.
pub const CLIENT_ID: &str = match option_env!("CLASSROOM_MS_CLIENT_ID") {
    Some(s) => s,
    None => "",
};

pub fn configured() -> bool {
    !CLIENT_ID.trim().is_empty()
}

/// Mode hors ligne : réservé au développement, activé par `CLASSROOM_DEV_OFFLINE=1`.
pub fn dev_offline() -> bool {
    crate::config::dev_env("CLASSROOM_DEV_OFFLINE").is_some_and(|v| v == "1")
}

const KEYRING_SERVICE: &str = "Opaline";
const KEYRING_USER: &str = "microsoft-refresh-token";
pub const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Clone)]
pub struct Session {
    pub username: String,
    pub uuid: String,
    /// "legacy" hors ligne ; "msa" avec Microsoft (absent du JSON 26.3 : sert seulement si un modèle d'arguments le réclame).
    pub user_type: &'static str,
    /// Vide : l'XUID n'est pas récupéré (valeur par défaut de `Main`).
    pub xuid: String,
    /// ID client de l'app Azure ; vide hors ligne.
    pub client_id: String,
    access_token: String,
}

impl Session {
    /// Session hors ligne (dev) : UUID déterministe comme le serveur vanilla hors ligne, token factice.
    pub fn offline(nickname: &str) -> Session {
        Session {
            username: nickname.to_string(),
            uuid: offline_uuid(nickname),
            user_type: "legacy",
            xuid: String::new(),
            client_id: String::new(),
            access_token: "0".into(),
        }
    }

    pub fn microsoft(username: String, uuid: String, access_token: String) -> Session {
        Session { username, uuid, user_type: "msa", xuid: String::new(), client_id: CLIENT_ID.to_string(), access_token }
    }

    pub fn access_token(&self) -> &str {
        &self.access_token
    }
}

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

/// URL complètes des services appelés (injectables pour simuler le flux).
#[derive(Clone, Debug)]
pub struct Endpoints {
    pub authorize: String,
    pub devicecode: String,
    pub token: String,
    pub xbl: String,
    pub xsts: String,
    pub mc_login: String,
    pub entitlements: String,
    pub profile: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Endpoints::with_bases(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0",
            "https://user.auth.xboxlive.com",
            "https://xsts.auth.xboxlive.com",
            "https://api.minecraftservices.com",
        )
    }
}

impl Endpoints {
    pub fn with_bases(ms: &str, xbl: &str, xsts: &str, mc: &str) -> Endpoints {
        let t = |s: &str| s.trim_end_matches('/').to_string();
        Endpoints {
            authorize: format!("{}/authorize", t(ms)),
            devicecode: format!("{}/devicecode", t(ms)),
            token: format!("{}/token", t(ms)),
            xbl: format!("{}/user/authenticate", t(xbl)),
            xsts: format!("{}/xsts/authorize", t(xsts)),
            mc_login: format!("{}/authentication/login_with_xbox", t(mc)),
            entitlements: format!("{}/entitlements/mcstore", t(mc)),
            profile: format!("{}/minecraft/profile", t(mc)),
        }
    }

    #[allow(dead_code)] // utilisé par les tests de simulation
    /// Toutes les URL derrière une seule base (serveur de fixtures).
    pub fn single_base(base: &str) -> Endpoints {
        let base = base.trim_end_matches('/');
        Endpoints::with_bases(&format!("{base}/ms"), &format!("{base}/xbl"), &format!("{base}/xsts"), &format!("{base}/mc"))
    }
}

// ---------- Logique pure : requêtes ----------

pub fn device_code_form(client_id: &str) -> Vec<(&'static str, String)> {
    vec![("client_id", client_id.into()), ("scope", SCOPE.into())]
}

pub fn poll_form(client_id: &str, device_code: &str) -> Vec<(&'static str, String)> {
    vec![("grant_type", "urn:ietf:params:oauth:grant-type:device_code".into()), ("client_id", client_id.into()), ("device_code", device_code.into())]
}

pub fn refresh_form(client_id: &str, refresh_token: &str) -> Vec<(&'static str, String)> {
    vec![("grant_type", "refresh_token".into()), ("client_id", client_id.into()), ("refresh_token", refresh_token.into()), ("scope", SCOPE.into())]
}

pub fn xbl_body(ms_access_token: &str) -> Value {
    json!({
        "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access_token}")},
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    })
}

pub fn xsts_body(xbl_token: &str) -> Value {
    json!({
        "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    })
}

pub fn mc_login_body(userhash: &str, xsts_token: &str) -> Value {
    json!({"identityToken": format!("XBL3.0 x={userhash};{xsts_token}")})
}

// ---------- Logique pure : réponses ----------

pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    device_code: String,
    pub expires_in: u64,
    pub interval: u64,
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

pub fn parse_device_code(v: &Value) -> Result<DeviceCode> {
    Ok(DeviceCode {
        user_code: str_of(v, "user_code").context("réponse devicecode sans user_code")?,
        verification_uri: str_of(v, "verification_uri").context("réponse devicecode sans verification_uri")?,
        device_code: str_of(v, "device_code").context("réponse devicecode sans device_code")?,
        expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(900),
        interval: v.get("interval").and_then(Value::as_u64).unwrap_or(5).max(1),
    })
}

/// Jetons Microsoft ; pas de `Debug`.
pub struct MsTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

pub enum Poll {
    Done(MsTokens),
    Pending,
    SlowDown,
    Declined,
    Expired,
    /// Code `error` OAuth (jamais de token).
    Failed(String),
}

pub fn parse_token_response(status: u16, v: &Value) -> Poll {
    if status == 200 {
        return match str_of(v, "access_token") {
            Some(access_token) => Poll::Done(MsTokens { access_token, refresh_token: str_of(v, "refresh_token") }),
            None => Poll::Failed("réponse sans access_token".into()),
        };
    }
    match v.get("error").and_then(Value::as_str).unwrap_or("") {
        "authorization_pending" => Poll::Pending,
        "slow_down" => Poll::SlowDown,
        "authorization_declined" => Poll::Declined,
        "expired_token" | "bad_verification_code" => Poll::Expired,
        "" => Poll::Failed(format!("HTTP {status}")),
        e => Poll::Failed(e.to_string()),
    }
}

pub fn xsts_error_message(code: u64) -> String {
    match code {
        2148916227 => "Ce compte est banni de Xbox Live.".into(),
        2148916233 => "Ce compte Microsoft n'a pas de compte Xbox : créez-en un sur xbox.com puis réessayez.".into(),
        2148916235 => "Xbox Live n'est pas disponible dans votre pays.".into(),
        2148916236 | 2148916237 => "Une vérification d'âge adulte est requise sur ce compte Xbox.".into(),
        2148916238 => "Compte enfant : il doit être rattaché à une famille Microsoft (autorisation d'un parent) pour jouer.".into(),
        c => format!("Erreur Xbox inconnue (XErr {c})."),
    }
}

/// (jeton, userhash) d'une réponse XBL ou XSTS.
pub fn parse_xbox_token(v: &Value) -> Result<(String, String)> {
    let token = str_of(v, "Token").context("réponse Xbox sans Token")?;
    let uhs = v.pointer("/DisplayClaims/xui/0/uhs").and_then(Value::as_str).context("réponse Xbox sans userhash")?.to_string();
    Ok((token, uhs))
}

/// Possession du jeu d'après `entitlements/mcstore`.
pub fn owns_game(v: &Value) -> bool {
    v.get("items")
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|i| matches!(i.get("name").and_then(Value::as_str), Some("product_minecraft" | "game_minecraft"))))
}

pub fn parse_profile(v: &Value) -> Result<(String, String)> {
    Ok((str_of(v, "name").context("profil sans nom")?, str_of(v, "id").context("profil sans id")?))
}

const NO_LICENSE: &str = "Ce compte Microsoft ne possède pas Minecraft : Java Edition.";
const NOT_APPROVED: &str = "Application Azure non encore approuvée par Mojang (403 sur login_with_xbox).";

// ---------- Réseau ----------

fn send(req: reqwest::blocking::RequestBuilder) -> Result<(u16, Value)> {
    let r = req.send().map_err(|e| anyhow!("erreur réseau : {}", e.without_url()))?;
    let status = r.status().as_u16();
    let text = r.text().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn post_form(c: &Client, url: &str, form: &[(&str, String)]) -> Result<(u16, Value)> {
    send(c.post(url).form(form))
}

fn post_json(c: &Client, url: &str, body: &Value) -> Result<(u16, Value)> {
    send(c.post(url).header("Accept", "application/json").json(body))
}

pub fn http_client() -> Result<Client> {
    Ok(Client::builder().user_agent(concat!("Opaline/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(30)).build()?)
}

pub fn start_device_code(c: &Client, ep: &Endpoints, client_id: &str) -> Result<DeviceCode> {
    let (st, v) = post_form(c, &ep.devicecode, &device_code_form(client_id))?;
    if st != 200 {
        let e = v.get("error").and_then(Value::as_str).unwrap_or("?");
        bail!("demande de code refusée (HTTP {st}, {e}) : vérifiez l'ID client de l'app Azure");
    }
    parse_device_code(&v)
}

/// Attend la validation de l'utilisateur. `cancel` à true => abandon.
pub fn poll_device_code(c: &Client, ep: &Endpoints, client_id: &str, dc: &DeviceCode, cancel: &AtomicBool) -> Result<MsTokens> {
    let deadline = Instant::now() + Duration::from_secs(dc.expires_in);
    let mut interval = dc.interval;
    loop {
        let wake = Instant::now() + Duration::from_secs(interval);
        while Instant::now() < wake {
            if cancel.load(Ordering::Relaxed) {
                bail!("connexion annulée");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if Instant::now() > deadline {
            bail!("code expiré : relancez la connexion");
        }
        // Une erreur réseau ponctuelle ne casse pas le flux : on réessaie jusqu'à l'échéance.
        let Ok((st, v)) = post_form(c, &ep.token, &poll_form(client_id, &dc.device_code)) else {
            continue;
        };
        match parse_token_response(st, &v) {
            Poll::Done(t) => return Ok(t),
            Poll::Pending => {}
            Poll::SlowDown => interval += 5,
            Poll::Declined => bail!("connexion refusée sur la page Microsoft"),
            Poll::Expired => bail!("code expiré : relancez la connexion"),
            Poll::Failed(e) => bail!("échec de la connexion Microsoft : {e}"),
        }
    }
}

/// Ok(None) = refresh token invalide ou révoqué (`invalid_grant`) : il faut se reconnecter.
pub fn refresh(c: &Client, ep: &Endpoints, client_id: &str, refresh_token: &str) -> Result<Option<MsTokens>> {
    let (st, v) = post_form(c, &ep.token, &refresh_form(client_id, refresh_token))?;
    match parse_token_response(st, &v) {
        Poll::Done(t) => Ok(Some(t)),
        Poll::Failed(e) if e == "invalid_grant" || e == "interaction_required" => Ok(None),
        Poll::Failed(e) => bail!("rafraîchissement refusé : {e}"),
        _ => bail!("rafraîchissement : réponse inattendue (HTTP {st})"),
    }
}

/// Jeton Microsoft -> XBL -> XSTS -> login_with_xbox -> possession du jeu -> profil.
pub fn minecraft_login(c: &Client, ep: &Endpoints, ms_access_token: &str) -> Result<Session> {
    let (st, v) = post_json(c, &ep.xbl, &xbl_body(ms_access_token))?;
    if st != 200 {
        bail!("Xbox Live (authentification) : HTTP {st}");
    }
    let (xbl_token, _) = parse_xbox_token(&v)?;
    let (st, v) = post_json(c, &ep.xsts, &xsts_body(&xbl_token))?;
    if st != 200 {
        match v.get("XErr").and_then(Value::as_u64) {
            Some(code) => bail!("{}", xsts_error_message(code)),
            None => bail!("XSTS : HTTP {st}"),
        }
    }
    let (xsts_token, uhs) = parse_xbox_token(&v)?;
    let (st, v) = post_json(c, &ep.mc_login, &mc_login_body(&uhs, &xsts_token))?;
    match st {
        200 => {}
        403 => bail!("{NOT_APPROVED}"),
        _ => bail!("login_with_xbox : HTTP {st}"),
    }
    let mc_token = str_of(&v, "access_token").context("login_with_xbox sans access_token")?;
    let (st, v) = send(c.get(&ep.entitlements).bearer_auth(&mc_token))?;
    if st != 200 {
        bail!("vérification de la licence : HTTP {st}");
    }
    if !owns_game(&v) {
        bail!("{NO_LICENSE}");
    }
    let (st, v) = send(c.get(&ep.profile).bearer_auth(&mc_token))?;
    match st {
        200 => {}
        404 => bail!("{NO_LICENSE}"),
        _ => bail!("profil Minecraft : HTTP {st}"),
    }
    let (name, id) = parse_profile(&v)?;
    Ok(Session::microsoft(name, id, mc_token))
}

// ---------- Trousseau système ----------

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| anyhow!("trousseau système indisponible : {e}"))
}

/// Stocké en octets (`set_secret`) : évite la limite de longueur des mots de passe texte du trousseau Windows.
pub fn store_refresh_token(token: &str) -> Result<()> {
    entry()?.set_secret(token.as_bytes()).map_err(|e| anyhow!("écriture dans le trousseau impossible : {e}"))
}

pub fn load_refresh_token() -> Result<Option<String>> {
    match entry()?.get_secret() {
        Ok(b) => Ok(String::from_utf8(b).ok()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => bail!("lecture du trousseau impossible : {e}"),
    }
}

/// Entrée héritée de l'ancien launcher : effacée à la déconnexion, jamais lue.
const LEGACY_KEYRING_SERVICE: &str = "ClassroomRunLauncher";

pub fn clear_legacy_token() -> Result<()> {
    let e = keyring::Entry::new(LEGACY_KEYRING_SERVICE, KEYRING_USER).map_err(|e| anyhow!("trousseau système indisponible : {e}"))?;
    match e.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => bail!("suppression du trousseau impossible : {e}"),
    }
}

/// Efface le refresh token du trousseau (sans toucher à la mémoire).
pub fn clear_stored_token() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => bail!("suppression du trousseau impossible : {e}"),
    }
}

// ---------- Gestionnaire de session ----------

/// Durée de vie conservée pour un jeton Minecraft en mémoire (il en vaut 24 h côté Mojang).
const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);

struct Inner {
    session: Option<(Session, Instant)>,
    refresh_token: Option<String>,
}

/// Garde la session et le refresh token en mémoire (jouer ne dépend pas du trousseau dans la même exécution).
/// `remember = false` : rien n'est écrit dans le trousseau (PC partagé). Pas de `Debug`.
pub struct AuthManager {
    inner: Mutex<Inner>,
    /// Incrémenté par `sign_out` : une connexion ou un rafraîchissement commencé avant ne stocke plus rien.
    generation: AtomicU64,
    warning: Mutex<Option<String>>,
}

impl Default for AuthManager {
    fn default() -> Self {
        AuthManager { inner: Mutex::new(Inner { session: None, refresh_token: None }), generation: AtomicU64::new(0), warning: Mutex::new(None) }
    }
}

impl AuthManager {
    /// Avertissement non bloquant (trousseau indisponible...), à afficher une fois.
    pub fn take_warning(&self) -> Option<String> {
        self.warning.lock().unwrap().take()
    }

    fn warn(&self, msg: String) {
        *self.warning.lock().unwrap() = Some(msg);
    }

    /// Jeton d'accès en cours (pour masquer les logs), s'il y en a un.
    pub fn current_token(&self) -> Option<String> {
        self.inner.lock().unwrap().session.as_ref().map(|(s, _)| s.access_token().to_string())
    }

    /// Mémorise les jetons si la génération n'a pas changé. Faux = déconnexion survenue entre-temps.
    fn commit(&self, gen_at_start: u64, refresh: Option<&str>, session: Option<&Session>, remember: bool) -> bool {
        let mut g = self.inner.lock().unwrap();
        if self.generation.load(Ordering::SeqCst) != gen_at_start {
            return false;
        }
        if let Some(r) = refresh {
            g.refresh_token = Some(r.to_string());
            if remember && let Err(e) = store_refresh_token(r) {
                self.warn(format!("{e} : la connexion ne sera pas mémorisée sur ce PC."));
            }
        }
        if let Some(s) = session {
            g.session = Some((s.clone(), Instant::now()));
        }
        true
    }

    /// Connexion interactive. `on_code` reçoit le code et l'URL à montrer à l'utilisateur.
    pub fn login(
        &self,
        c: &Client,
        ep: &Endpoints,
        client_id: &str,
        remember: bool,
        on_code: impl FnOnce(&DeviceCode),
        cancel: &AtomicBool,
    ) -> Result<Session> {
        let gen_at_start = self.generation.load(Ordering::SeqCst);
        let dc = start_device_code(c, ep, client_id)?;
        on_code(&dc);
        let t = poll_device_code(c, ep, client_id, &dc, cancel)?;
        self.finish_login(c, ep, gen_at_start, &t, remember)
    }

    /// Connexion par le navigateur (code d'autorisation + PKCE). `on_url` reçoit l'URL à ouvrir ; l'attente de la
    /// redirection locale est annulable par `cancel` et plafonnée par `timeout`.
    #[allow(clippy::too_many_arguments)]
    pub fn login_browser(
        &self,
        c: &Client,
        ep: &Endpoints,
        client_id: &str,
        remember: bool,
        on_url: impl FnOnce(&str),
        cancel: &AtomicBool,
        timeout: Duration,
    ) -> Result<Session> {
        use crate::auth_browser as b;
        let gen_at_start = self.generation.load(Ordering::SeqCst);
        let (verifier, state) = (b::new_verifier()?, b::new_state()?);
        let (listener, redirect_uri) = b::bind_loopback()?;
        on_url(&b::authorize_url(&ep.authorize, client_id, &redirect_uri, &state, &b::pkce_challenge(&verifier))?);
        let code = match b::wait_for_redirect(listener, &state, cancel, timeout)? {
            b::Redirect::Code(code) => code,
            b::Redirect::Denied(e) if e == "access_denied" => bail!("connexion refusée sur la page Microsoft"),
            b::Redirect::Denied(e) => bail!("échec de la connexion Microsoft : {e}"),
        };
        let (st, v) = post_form(c, &ep.token, &b::code_form(client_id, &code, &redirect_uri, &verifier))?;
        let t = match parse_token_response(st, &v) {
            Poll::Done(t) => t,
            Poll::Failed(e) => bail!("échange du code refusé : {e}"),
            _ => bail!("échange du code : réponse inattendue"),
        };
        self.finish_login(c, ep, gen_at_start, &t, remember)
    }

    fn finish_login(&self, c: &Client, ep: &Endpoints, gen_at_start: u64, t: &MsTokens, remember: bool) -> Result<Session> {
        if !self.commit(gen_at_start, t.refresh_token.as_deref(), None, remember) {
            bail!("connexion annulée par une déconnexion");
        }
        let session = minecraft_login(c, ep, &t.access_token)?;
        if !self.commit(gen_at_start, None, Some(&session), remember) {
            bail!("connexion annulée par une déconnexion");
        }
        Ok(session)
    }

    /// Session valide : en mémoire, sinon rafraîchie (refresh token en mémoire, ou trousseau si `remember`).
    /// Ok(None) = pas de session (à reconnecter) ; un refresh token rejeté est effacé.
    pub fn ensure_session(&self, c: &Client, ep: &Endpoints, client_id: &str, remember: bool) -> Result<Option<Session>> {
        let gen_at_start = self.generation.load(Ordering::SeqCst);
        let mem_rt = {
            let g = self.inner.lock().unwrap();
            if let Some((s, at)) = &g.session
                && at.elapsed() < SESSION_TTL
            {
                return Ok(Some(s.clone()));
            }
            g.refresh_token.clone()
        };
        let rt = match mem_rt {
            Some(r) => r,
            None if remember => match load_refresh_token() {
                Ok(Some(r)) => r,
                Ok(None) => return Ok(None),
                Err(e) => {
                    self.warn(format!("{e}"));
                    return Ok(None);
                }
            },
            None => return Ok(None),
        };
        let Some(t) = refresh(c, ep, client_id, &rt)? else {
            let mut g = self.inner.lock().unwrap();
            if self.generation.load(Ordering::SeqCst) == gen_at_start {
                g.refresh_token = None;
                g.session = None;
                let _ = clear_stored_token();
            }
            return Ok(None);
        };
        if !self.commit(gen_at_start, t.refresh_token.as_deref(), None, remember) {
            return Ok(None);
        }
        let session = minecraft_login(c, ep, &t.access_token)?;
        Ok(self.commit(gen_at_start, None, Some(&session), remember).then_some(session))
    }

    /// Déconnexion : efface d'abord le trousseau, puis la mémoire (vidée même si le trousseau échoue) ;
    /// empêche tout stockage en cours.
    pub fn sign_out(&self) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        self.generation.fetch_add(1, Ordering::SeqCst);
        let r = clear_stored_token().and(clear_legacy_token());
        g.session = None;
        g.refresh_token = None;
        r
    }
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
        let s = Session::microsoft("A".into(), "u".into(), "secret-token".into());
        assert!(!format!("{s:?}").contains("secret-token"));
        assert!(format!("{s:?}").contains("masqué"));
    }

    #[test]
    fn requests_have_the_documented_shape() {
        let f = poll_form("cid", "dev");
        assert!(f.contains(&("grant_type", "urn:ietf:params:oauth:grant-type:device_code".into())) && f.contains(&("device_code", "dev".into())));
        assert!(device_code_form("c").contains(&("scope", "XboxLive.signin offline_access".into())));
        assert!(refresh_form("c", "r").contains(&("grant_type", "refresh_token".into())));
        assert_eq!(xbl_body("T")["Properties"]["RpsTicket"], "d=T");
        assert_eq!(xsts_body("X")["Properties"]["UserTokens"][0], "X");
        assert_eq!(xsts_body("X")["RelyingParty"], "rp://api.minecraftservices.com/");
        assert_eq!(mc_login_body("H", "S")["identityToken"], "XBL3.0 x=H;S");
    }

    #[test]
    fn token_responses_are_classified() {
        let err = |e: &str| json!({"error": e});
        assert!(matches!(parse_token_response(400, &err("authorization_pending")), Poll::Pending));
        assert!(matches!(parse_token_response(400, &err("slow_down")), Poll::SlowDown));
        assert!(matches!(parse_token_response(400, &err("authorization_declined")), Poll::Declined));
        assert!(matches!(parse_token_response(400, &err("expired_token")), Poll::Expired));
        assert!(matches!(parse_token_response(400, &err("invalid_grant")), Poll::Failed(e) if e == "invalid_grant"));
        let Poll::Done(t) = parse_token_response(200, &json!({"access_token": "a", "refresh_token": "r"})) else { panic!() };
        assert_eq!((t.access_token.as_str(), t.refresh_token.as_deref()), ("a", Some("r")));
        assert!(matches!(parse_token_response(200, &json!({})), Poll::Failed(_)));
    }

    #[test]
    fn xsts_errors_are_mapped() {
        assert!(xsts_error_message(2148916233).contains("pas de compte Xbox"));
        assert!(xsts_error_message(2148916238).contains("Compte enfant"));
        assert!(xsts_error_message(1).contains("XErr 1"));
    }

    #[test]
    fn parses_xbox_entitlements_and_profile() {
        let v = json!({"Token": "t", "DisplayClaims": {"xui": [{"uhs": "h"}]}});
        assert_eq!(parse_xbox_token(&v).unwrap(), ("t".into(), "h".into()));
        assert!(parse_xbox_token(&json!({"Token": "t"})).is_err());
        assert!(owns_game(&json!({"items": [{"name": "game_minecraft"}]})));
        assert!(!owns_game(&json!({"items": []})) && !owns_game(&json!({})));
        assert_eq!(parse_profile(&json!({"id": "i", "name": "n"})).unwrap(), ("n".into(), "i".into()));
        let dc = parse_device_code(&json!({"user_code": "AB", "verification_uri": "https://x", "device_code": "d", "expires_in": 900, "interval": 5})).unwrap();
        assert_eq!((dc.user_code.as_str(), dc.interval), ("AB", 5));
    }

    #[test]
    fn endpoints_are_injectable() {
        let e = Endpoints::single_base("http://127.0.0.1:9/");
        assert_eq!(e.token, "http://127.0.0.1:9/ms/token");
        assert_eq!(e.mc_login, "http://127.0.0.1:9/mc/authentication/login_with_xbox");
        assert!(Endpoints::default().token.contains("/consumers/oauth2/v2.0/token"));
    }
}
