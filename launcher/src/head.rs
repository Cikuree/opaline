//! Tête d'un joueur : profil `sessionserver.mojang.com`, texture `textures.minecraft.net` (autre hôte refusé),
//! recadrage du visage 8×8 + calque du chapeau, cache disque.
use crate::config::Dirs;
use crate::skins::fetch_texture;
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::{Duration, SystemTime};

const CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
pub const SESSION_SERVER: &str = "https://sessionserver.mojang.com";

/// UUID en 32 hexadécimaux minuscules (accepte la forme avec tirets).
pub fn normalize_uuid(u: &str) -> Result<String> {
    let s: String = u.trim().chars().filter(|c| *c != '-').collect::<String>().to_ascii_lowercase();
    if s.len() != 32 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("UUID invalide");
    }
    Ok(s)
}

/// URL de la texture de skin d'après la propriété `textures` (base64) du profil.
pub fn skin_url_from_profile(profile: &Value) -> Result<String> {
    let value = profile
        .get("properties")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().find(|p| p.get("name").and_then(Value::as_str) == Some("textures")))
        .and_then(|p| p.get("value"))
        .and_then(Value::as_str)
        .context("profil sans texture")?;
    let raw = base64::engine::general_purpose::STANDARD.decode(value).context("texture illisible")?;
    let v: Value = serde_json::from_slice(&raw).context("texture illisible")?;
    let url = v.pointer("/textures/SKIN/url").and_then(Value::as_str).context("profil sans skin")?;
    crate::skins::official_texture_url(url).context("adresse de texture refusée (hôte non officiel)")
}

/// RGBA 8 bits d'un PNG de skin (64×64 ou 64×32).
fn decode_rgba(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().map_err(|_| anyhow!("skin illisible"))?;
    let mut buf = vec![0; r.output_buffer_size().context("skin invalide")?];
    let f = r.next_frame(&mut buf).map_err(|_| anyhow!("skin illisible"))?;
    let (w, h) = (f.width, f.height);
    if w != 64 || !(h == 64 || h == 32) {
        bail!("skin de dimensions inattendues");
    }
    let px = &buf[..f.buffer_size()];
    let rgba: Vec<u8> = match f.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => bail!("skin indexé non pris en charge"),
    };
    Ok((w, h, rgba))
}

/// Visage (8,8) + chapeau (40,8), 8×8 chacun, chapeau composé par-dessus (alpha). Sortie : PNG RGBA 8×8.
pub fn crop_head(skin_png: &[u8]) -> Result<Vec<u8>> {
    let (w, _h, px) = decode_rgba(skin_png)?;
    let at = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    let mut out = Vec::with_capacity(8 * 8 * 4);
    for y in 0..8 {
        for x in 0..8 {
            let (f, hat) = (at(8 + x, 8 + y), at(40 + x, 8 + y));
            let a = hat[3] as u32;
            // Le visage est toujours opaque dans le jeu.
            for k in 0..3 {
                out.push(((hat[k] as u32 * a + f[k] as u32 * (255 - a)) / 255) as u8);
            }
            out.push(255);
        }
    }
    let mut png_out = Vec::new();
    let mut e = png::Encoder::new(&mut png_out, 8, 8);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()?.write_image_data(&out)?;
    Ok(png_out)
}

/// PNG décodable de 8×8 pixels.
fn valid_head(b: &[u8]) -> bool {
    let Ok(mut r) = png::Decoder::new(std::io::Cursor::new(b)).read_info() else { return false };
    if (r.info().width, r.info().height) != (8, 8) {
        return false;
    }
    let Some(n) = r.output_buffer_size() else { return false };
    r.next_frame(&mut vec![0; n]).is_ok()
}

pub fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

/// Tête en data URL, depuis le cache (24 h) ou le réseau ; cache périmé utilisé si le réseau échoue.
pub fn player_head(dirs: &Dirs, client: &Client, uuid: &str, session_server: &str) -> Result<String> {
    let id = normalize_uuid(uuid)?;
    let dir = dirs.cache().join("heads");
    let file = dir.join(format!("{id}.png"));
    // Un cache illisible ou de mauvaise taille est ignoré (puis réécrit après rechargement).
    let cached = std::fs::read(&file).ok().filter(|b| valid_head(b));
    let fresh =
        std::fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|age| age < CACHE_TTL);
    if let (true, Some(b)) = (fresh, &cached) {
        return Ok(data_url(b));
    }
    let fetched = (|| -> Result<Vec<u8>> {
        let r = client
            .get(format!("{}/session/minecraft/profile/{id}", session_server.trim_end_matches('/')))
            .send()
            .map_err(|_| anyhow!("profil indisponible (réseau)"))?;
        if !r.status().is_success() {
            bail!("profil indisponible (HTTP {})", r.status().as_u16());
        }
        let profile: Value = r.json().map_err(|_| anyhow!("profil illisible"))?;
        crop_head(&fetch_texture(client, &skin_url_from_profile(&profile)?)?)
    })();
    match (fetched, cached) {
        (Ok(head), _) => {
            if std::fs::create_dir_all(&dir).is_ok() {
                let _ = std::fs::write(&file, &head);
            }
            Ok(data_url(&head))
        }
        (Err(_), Some(old)) => Ok(data_url(&old)),
        (Err(e), None) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skin synthétique 64×h : visage rouge, chapeau vert opaque sur la 1re colonne et transparent ailleurs.
    fn skin(h: u32) -> Vec<u8> {
        let mut px = vec![0u8; (64 * h * 4) as usize];
        for y in 0..8 {
            for x in 0..8 {
                let f = (((8 + y) * 64 + 8 + x) * 4) as usize;
                px[f..f + 4].copy_from_slice(&[200, 0, 0, 255]);
                if x == 0 {
                    let g = (((8 + y) * 64 + 40 + x) * 4) as usize;
                    px[g..g + 4].copy_from_slice(&[0, 200, 0, 255]);
                }
            }
        }
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, 64, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
        out
    }

    fn decode_8x8(head: &[u8]) -> Vec<u8> {
        let mut r = png::Decoder::new(std::io::Cursor::new(head)).read_info().unwrap();
        assert_eq!((r.info().width, r.info().height), (8, 8));
        let mut b = vec![0; r.output_buffer_size().unwrap()];
        r.next_frame(&mut b).unwrap();
        b
    }

    #[test]
    fn crops_face_and_overlays_hat() {
        for h in [64, 32] {
            let px = decode_8x8(&crop_head(&skin(h)).unwrap());
            assert_eq!(&px[..4], &[0, 200, 0, 255], "chapeau opaque sur x=0");
            assert_eq!(&px[4 * 4..4 * 4 + 4], &[200, 0, 0, 255], "visage ailleurs");
        }
    }

    #[test]
    fn rejects_bad_skins_and_uuids() {
        assert!(crop_head(b"nope").is_err());
        assert_eq!(normalize_uuid("B50AD385-829D-3141-A216-7E7D7539BA7F").unwrap(), "b50ad385829d3141a2167e7d7539ba7f");
        assert!(normalize_uuid("../x").is_err() && normalize_uuid("abc").is_err());
    }

    #[test]
    fn profile_texture_host_is_checked() {
        let mk = |url: &str| {
            let t = base64::engine::general_purpose::STANDARD.encode(format!(r#"{{"textures":{{"SKIN":{{"url":"{url}"}}}}}}"#));
            serde_json::json!({"properties": [{"name": "textures", "value": t}]})
        };
        assert_eq!(skin_url_from_profile(&mk("https://textures.minecraft.net/texture/x")).unwrap(), "https://textures.minecraft.net/texture/x");
        assert_eq!(skin_url_from_profile(&mk("http://textures.minecraft.net/texture/y")).unwrap(), "https://textures.minecraft.net/texture/y");
        assert!(skin_url_from_profile(&mk("https://evil.example/texture/x")).is_err());
        assert!(skin_url_from_profile(&serde_json::json!({})).is_err());
    }
}
