//! Skins et capes : garde-robe locale (`<data>/skins/`) et API officielle Mojang (jeton de session obligatoire).
//! Aucune requête sans jeton ; les erreurs ne citent jamais le jeton ni un corps de réponse.
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;

pub const MAX_SKIN_BYTES: usize = 256 * 1024;
const MAX_NAME_CHARS: usize = 24;
/// Seul hôte autorisé pour télécharger une texture de skin ou de cape.
pub const TEXTURE_HOST: &str = "textures.minecraft.net";

#[derive(Clone, Debug)]
pub struct Endpoints {
    pub profile: String,
    pub skins: String,
    pub capes_active: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self::with_base("https://api.minecraftservices.com")
    }
}

impl Endpoints {
    pub fn with_base(base: &str) -> Endpoints {
        let b = base.trim_end_matches('/');
        Endpoints {
            profile: format!("{b}/minecraft/profile"),
            skins: format!("{b}/minecraft/profile/skins"),
            capes_active: format!("{b}/minecraft/profile/capes/active"),
        }
    }
}

// ---------- Validation et formats ----------

/// PNG réel (décodé en entier) de 64×64 ou 64×32. Renvoie (largeur, hauteur).
pub fn validate_png(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() > MAX_SKIN_BYTES {
        bail!("image trop lourde (256 Ko maximum)");
    }
    let mut r = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().map_err(|_| anyhow!("ce fichier n'est pas un PNG valide"))?;
    let (w, h) = (r.info().width, r.info().height);
    if w != 64 || !(h == 64 || h == 32) {
        bail!("dimensions {w}×{h} refusées : un skin fait 64×64 ou 64×32");
    }
    let mut buf = vec![0; r.output_buffer_size().context("image invalide")?];
    r.next_frame(&mut buf).map_err(|_| anyhow!("ce fichier n'est pas un PNG valide"))?;
    Ok((w, h))
}

pub fn parse_model(m: &str) -> Result<&'static str> {
    match m {
        "classic" => Ok("classic"),
        "slim" => Ok("slim"),
        _ => bail!("modèle de skin inconnu (classic ou slim)"),
    }
}

fn clean_name(n: &str) -> String {
    let s: String = n.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(MAX_NAME_CHARS).collect();
    if s.is_empty() { "Skin".into() } else { s }
}

/// Corps `multipart/form-data` de `POST /minecraft/profile/skins` : champs `variant` puis `file`.
pub fn multipart_body(boundary: &str, variant: &str, png: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n").as_bytes());
    b.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes(),
    );
    b.extend_from_slice(png);
    b.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    b
}

/// Mojang publie souvent `http://textures.minecraft.net/...` : même hôte officiel, remis en https. Autre hôte : `None`.
pub fn official_texture_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let ok = matches!(u.scheme(), "http" | "https") && u.host_str() == Some(TEXTURE_HOST) && u.port().is_none() && u.username().is_empty();
    ok.then(|| format!("https://{TEXTURE_HOST}{}", u.path()))
}

pub fn is_texture_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| u.scheme() == "https" && u.host_str() == Some(TEXTURE_HOST) && u.port().is_none() && u.username().is_empty())
}

/// Télécharge une texture (hôte officiel uniquement, taille bornée).
pub fn fetch_texture(client: &Client, url: &str) -> Result<Vec<u8>> {
    if !is_texture_url(url) {
        bail!("adresse de texture refusée (hôte non officiel)");
    }
    let r = client.get(url).send().map_err(|_| anyhow!("téléchargement de la texture impossible"))?;
    if !r.status().is_success() {
        bail!("texture indisponible (HTTP {})", r.status().as_u16());
    }
    let mut v = Vec::new();
    r.take(MAX_SKIN_BYTES as u64 + 1).read_to_end(&mut v)?;
    if v.len() > MAX_SKIN_BYTES {
        bail!("texture trop lourde");
    }
    Ok(v)
}

// ---------- Garde-robe ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub model: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Index {
    skins: Vec<Entry>,
    active: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkinView {
    pub id: String,
    pub name: String,
    pub model: String,
    pub active: bool,
    pub data_url: String,
}

/// Verrou de l'index : toute lecture-modification-écriture est atomique (commandes concurrentes de l'interface).
static INDEX_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn locked() -> std::sync::MutexGuard<'static, ()> {
    INDEX_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct Wardrobe {
    dir: PathBuf,
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

impl Wardrobe {
    pub fn new(dir: PathBuf) -> Wardrobe {
        Wardrobe { dir }
    }

    fn index(&self) -> Index {
        std::fs::read(self.dir.join("index.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save(&self, i: &Index) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join("index.json.part");
        std::fs::write(&tmp, serde_json::to_vec_pretty(i)?)?;
        std::fs::rename(&tmp, self.dir.join("index.json"))?;
        Ok(())
    }

    /// Id = début du SHA-256 du PNG : le même fichier n'est stocké qu'une fois. Nom de fichier sûr par construction.
    fn id_of(png: &[u8]) -> String {
        hex::encode(Sha256::digest(png))[..16].to_string()
    }

    fn png_path(&self, id: &str) -> Result<PathBuf> {
        if id.len() != 16 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("skin inconnu");
        }
        Ok(self.dir.join(format!("{id}.png")))
    }

    fn view(&self, e: &Entry, active: &Option<String>) -> Option<SkinView> {
        let png = std::fs::read(self.png_path(&e.id).ok()?).ok()?;
        Some(SkinView {
            id: e.id.clone(),
            name: e.name.clone(),
            model: e.model.clone(),
            active: active.as_deref() == Some(e.id.as_str()),
            data_url: data_url(&png),
        })
    }

    pub fn list(&self) -> Vec<SkinView> {
        let i = self.index();
        i.skins.iter().filter_map(|e| self.view(e, &i.active)).collect()
    }

    pub fn import(&self, name: &str, model: &str, png: &[u8]) -> Result<SkinView> {
        let _g = locked();
        self.import_unlocked(name, model, png)
    }

    fn import_unlocked(&self, name: &str, model: &str, png: &[u8]) -> Result<SkinView> {
        validate_png(png)?;
        let model = parse_model(model)?;
        let mut i = self.index();
        let id = Self::id_of(png);
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.png_path(&id)?, png)?;
        let e = Entry { id: id.clone(), name: clean_name(name), model: model.into() };
        match i.skins.iter_mut().find(|s| s.id == id) {
            Some(s) => *s = e.clone(),
            None => i.skins.push(e.clone()),
        }
        self.save(&i)?;
        self.view(&e, &i.active).context("skin illisible après import")
    }

    pub fn get(&self, id: &str) -> Result<(Entry, Vec<u8>)> {
        let e = self.index().skins.into_iter().find(|s| s.id == id).context("skin inconnu")?;
        let png = std::fs::read(self.png_path(id)?).context("fichier du skin absent")?;
        Ok((e, png))
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<()> {
        let _g = locked();
        let mut i = self.index();
        i.skins.iter_mut().find(|s| s.id == id).context("skin inconnu")?.name = clean_name(name);
        self.save(&i)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let _g = locked();
        let mut i = self.index();
        let before = i.skins.len();
        i.skins.retain(|s| s.id != id);
        if i.skins.len() == before {
            bail!("skin inconnu");
        }
        if i.active.as_deref() == Some(id) {
            i.active = None;
        }
        let _ = std::fs::remove_file(self.png_path(id)?);
        self.save(&i)
    }

    pub fn set_active(&self, id: Option<&str>) -> Result<()> {
        let _g = locked();
        self.set_active_unlocked(id)
    }

    fn set_active_unlocked(&self, id: Option<&str>) -> Result<()> {
        let mut i = self.index();
        i.active = id.map(str::to_string);
        self.save(&i)
    }

    /// Ajoute le skin actuel du compte (s'il n'y est pas déjà) et le marque actif.
    pub fn adopt_current(&self, png: &[u8], model: &str) -> Result<()> {
        let _g = locked();
        let id = Self::id_of(png);
        if !self.index().skins.iter().any(|s| s.id == id) {
            self.import_unlocked("Skin actuel", model, png)?;
        }
        self.set_active_unlocked(Some(&id))
    }
}

// ---------- API Mojang ----------

fn token_required(token: &str) -> Result<()> {
    if token.trim().is_empty() {
        bail!("connexion Microsoft requise");
    }
    Ok(())
}

fn check(r: reqwest::blocking::Response, what: &str) -> Result<Value> {
    let st = r.status();
    match st.as_u16() {
        200..=299 => Ok(r.json().unwrap_or(Value::Null)),
        401 | 403 => bail!("session expirée : reconnectez-vous ({what})"),
        429 => bail!("trop de requêtes : réessayez dans une minute ({what})"),
        c => bail!("{what} : HTTP {c}"),
    }
}

fn net<T>(r: reqwest::Result<T>, what: &str) -> Result<T> {
    r.map_err(|_| anyhow!("{what} : réseau indisponible"))
}

pub fn get_profile(c: &Client, ep: &Endpoints, token: &str) -> Result<Value> {
    token_required(token)?;
    check(net(c.get(&ep.profile).bearer_auth(token).send(), "profil")?, "profil")
}

/// Skin actif du profil : (url, modèle).
pub fn current_skin(profile: &Value) -> Option<(String, &'static str)> {
    let s = profile.get("skins")?.as_array()?.iter().find(|s| s.get("state").and_then(Value::as_str) == Some("ACTIVE"))?;
    let model = if s.get("variant").and_then(Value::as_str).is_some_and(|v| v.eq_ignore_ascii_case("slim")) { "slim" } else { "classic" };
    Some((official_texture_url(s.get("url")?.as_str()?)?, model))
}

pub fn apply_skin(c: &Client, ep: &Endpoints, token: &str, model: &str, png: &[u8]) -> Result<()> {
    token_required(token)?;
    validate_png(png)?;
    let boundary = format!("----opaline{}", &hex::encode(Sha256::digest(png))[..24]);
    let body = multipart_body(&boundary, parse_model(model)?, png);
    let r = c.post(&ep.skins).bearer_auth(token).header("Content-Type", format!("multipart/form-data; boundary={boundary}")).body(body).send();
    check(net(r, "changement de skin")?, "changement de skin").map(|_| ())
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CapeView {
    pub id: String,
    pub alias: String,
    pub active: bool,
    pub url: String,
}

pub fn parse_capes(profile: &Value) -> Vec<CapeView> {
    profile
        .get("capes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(CapeView {
                        id: c.get("id")?.as_str()?.to_string(),
                        alias: c.get("alias").and_then(Value::as_str).unwrap_or("Cape").to_string(),
                        active: c.get("state").and_then(Value::as_str) == Some("ACTIVE"),
                        url: c.get("url").and_then(Value::as_str).and_then(official_texture_url).unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `None` masque la cape (`DELETE`), `Some(id)` l'active (`PUT {"capeId"}`).
pub fn set_cape(c: &Client, ep: &Endpoints, token: &str, id: Option<&str>) -> Result<()> {
    token_required(token)?;
    let r = match id {
        Some(id) => {
            if !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') || id.is_empty() {
                bail!("cape inconnue");
            }
            c.put(&ep.capes_active).bearer_auth(token).json(&serde_json::json!({ "capeId": id })).send()
        }
        None => c.delete(&ep.capes_active).bearer_auth(token).send(),
    };
    check(net(r, "changement de cape")?, "changement de cape").map(|_| ())
}

/// À la connexion : le skin actuel du compte rejoint la garde-robe (échec silencieux côté appelant).
pub fn sync_current(c: &Client, ep: &Endpoints, token: &str, w: &Wardrobe) -> Result<()> {
    let profile = get_profile(c, ep, token)?;
    let Some((url, model)) = current_skin(&profile) else {
        return Ok(());
    };
    let png = fetch_texture(c, &url)?;
    w.adopt_current(&png, model)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn skin_png(w: u32, h: u32, seed: u8) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(&vec![seed; (w * h * 4) as usize]).unwrap();
        drop(wr);
        out
    }

    #[test]
    fn png_validation() {
        assert_eq!(validate_png(&skin_png(64, 64, 1)).unwrap(), (64, 64));
        assert_eq!(validate_png(&skin_png(64, 32, 1)).unwrap(), (64, 32));
        assert!(validate_png(&skin_png(32, 32, 1)).is_err());
        assert!(validate_png(&skin_png(128, 128, 1)).is_err());
        assert!(validate_png(b"GIF89a pas un png").is_err());
        let mut truncated = skin_png(64, 64, 1);
        truncated.truncate(truncated.len() / 2);
        assert!(validate_png(&truncated).is_err());
        assert!(validate_png(&vec![0; MAX_SKIN_BYTES + 1]).is_err());
    }

    #[test]
    fn multipart_has_variant_then_file() {
        let b = multipart_body("BND", "slim", b"PNGDATA");
        let t = String::from_utf8_lossy(&b).into_owned();
        assert!(t.starts_with("--BND\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\nslim\r\n--BND\r\n"));
        assert!(t.contains("name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\nPNGDATA\r\n--BND--\r\n"));
    }

    #[test]
    fn texture_host_is_strict() {
        assert!(is_texture_url("https://textures.minecraft.net/texture/abc"));
        for u in [
            "http://textures.minecraft.net/texture/a",
            "https://evil.com/texture/a",
            "https://textures.minecraft.net.evil.com/a",
            "https://textures.minecraft.net@evil.com/a",
            "https://textures.minecraft.net:8443/a",
            "nope",
        ] {
            assert!(!is_texture_url(u), "{u}");
        }
    }

    #[test]
    fn wardrobe_roundtrip() {
        let d = std::env::temp_dir().join(format!("opaline-skins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let w = Wardrobe::new(d.clone());
        assert!(w.import("x", "weird", &skin_png(64, 64, 1)).is_err());
        assert!(w.import("x", "classic", b"nope").is_err());
        let v = w.import("  Mon skin\u{7} ", "slim", &skin_png(64, 64, 1)).unwrap();
        assert_eq!((v.name.as_str(), v.model.as_str(), v.active), ("Mon skin", "slim", false));
        assert!(v.data_url.starts_with("data:image/png;base64,"));
        assert_eq!(w.import("bis", "slim", &skin_png(64, 64, 1)).unwrap().id, v.id);
        assert_eq!(w.list().len(), 1);
        w.adopt_current(&skin_png(64, 32, 2), "classic").unwrap();
        let l = w.list();
        assert_eq!(l.len(), 2);
        assert_eq!(l.iter().filter(|s| s.active).count(), 1);
        w.rename(&v.id, "Autre").unwrap();
        assert!(w.rename("../x", "a").is_err() && w.delete("0000000000000000").is_err());
        w.delete(&v.id).unwrap();
        assert_eq!(w.list().len(), 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn requests_need_a_token() {
        let c = Client::new();
        let ep = Endpoints::with_base("http://127.0.0.1:9");
        assert!(get_profile(&c, &ep, "").unwrap_err().to_string().contains("requise"));
        assert!(apply_skin(&c, &ep, " ", "classic", &skin_png(64, 64, 1)).is_err());
        assert!(set_cape(&c, &ep, "", None).is_err());
    }

    #[test]
    fn parses_profile_skin_and_capes() {
        let p: Value = serde_json::from_str(
            r#"{"skins":[{"state":"INACTIVE","url":"https://textures.minecraft.net/a","variant":"CLASSIC"},{"state":"ACTIVE","url":"https://textures.minecraft.net/b","variant":"SLIM"}],
            "capes":[{"id":"c1","state":"ACTIVE","url":"https://textures.minecraft.net/cape","alias":"Migrator"},{"id":"c2","state":"INACTIVE","url":"https://evil/x","alias":"Y"}]}"#,
        )
        .unwrap();
        assert_eq!(current_skin(&p), Some(("https://textures.minecraft.net/b".into(), "slim")));
        assert_eq!(official_texture_url("http://textures.minecraft.net/texture/x").as_deref(), Some("https://textures.minecraft.net/texture/x"));
        assert!(official_texture_url("http://textures.minecraft.net.evil.com/x").is_none());
        let c = parse_capes(&p);
        assert_eq!((c[0].active, c[1].active, c[1].url.as_str()), (true, false, ""));
        assert!(current_skin(&serde_json::json!({})).is_none());
    }
}
