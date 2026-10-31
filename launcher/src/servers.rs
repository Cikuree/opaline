//! Liste Multijoueur de l'instance (`servers.dat`, NBT non compressé comme `NbtIo.write` de Minecraft) : ajout d'une
//! entrée sans jamais toucher aux autres. Lecteur/écrivain NBT minimal mais sans perte (tous les types sont conservés).
use anyhow::{Result, bail};
use std::path::Path;

const MAX_SIZE: u64 = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    /// Byte, short, int, long, float, double et tableaux : charge utile brute (longueur incluse pour les tableaux).
    Raw(u8, Vec<u8>),
    Str(Vec<u8>),
    List(u8, Vec<Tag>),
    Compound(Vec<(Vec<u8>, Tag)>),
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.0.len() {
            bail!("NBT tronqué");
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn len(&mut self) -> Result<usize> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into()?).max(0) as usize)
    }
    fn string(&mut self) -> Result<Vec<u8>> {
        let n = u16::from_be_bytes(self.take(2)?.try_into()?) as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn payload(&mut self, id: u8, depth: usize) -> Result<Tag> {
        if depth > MAX_DEPTH {
            bail!("NBT trop profond");
        }
        Ok(match id {
            1..=6 => Tag::Raw(id, self.take([1, 2, 4, 8, 4, 8][id as usize - 1])?.to_vec()),
            7 | 11 | 12 => {
                let head = self.take(4)?;
                let n = i32::from_be_bytes(head.try_into()?).max(0) as usize;
                let size = match id {
                    7 => 1,
                    11 => 4,
                    _ => 8,
                };
                let body = self.take(n.saturating_mul(size))?;
                Tag::Raw(id, [head, body].concat())
            }
            8 => Tag::Str(self.string()?),
            9 => {
                let t = self.u8()?;
                let n = self.len()?;
                if (t == 0 && n > 0) || t > 12 {
                    bail!("liste NBT invalide");
                }
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(self.payload(t, depth + 1)?);
                }
                Tag::List(t, v)
            }
            10 => {
                let mut v = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    let name = self.string()?;
                    v.push((name, self.payload(t, depth + 1)?));
                }
                Tag::Compound(v)
            }
            _ => bail!("type NBT inconnu"),
        })
    }
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Raw(i, _) => *i,
            Tag::Str(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
        }
    }
    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Tag::Raw(_, b) => out.extend_from_slice(b),
            Tag::Str(s) => write_str(out, s),
            Tag::List(t, v) => {
                out.push(*t);
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                v.iter().for_each(|x| x.write(out));
            }
            Tag::Compound(v) => {
                for (k, x) in v {
                    out.push(x.id());
                    write_str(out, k);
                    x.write(out);
                }
                out.push(0);
            }
        }
    }
    fn str_of(&self, key: &str) -> Option<&[u8]> {
        let Tag::Compound(v) = self else { return None };
        v.iter().find_map(|(k, x)| match x {
            Tag::Str(s) if k == key.as_bytes() => Some(s.as_slice()),
            _ => None,
        })
    }
}

fn write_str(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s);
}

/// Fichier NBT : compound racine (nom ignoré) ; toute anomalie (dont octets en trop) est une erreur.
pub fn decode(bytes: &[u8]) -> Result<Tag> {
    let mut r = Reader(bytes);
    if r.u8()? != 10 {
        bail!("racine NBT inattendue");
    }
    r.string()?;
    let t = r.payload(10, 0)?;
    if !r.0.is_empty() {
        bail!("octets en trop après le NBT");
    }
    Ok(t)
}

/// Racine sans nom, comme le fait le jeu.
pub fn encode(root: &Tag) -> Vec<u8> {
    let mut out = vec![10, 0, 0];
    root.write(&mut out);
    out
}

fn new_entry(name: &str, ip: &str) -> Tag {
    let s = |k: &str, v: &str| (k.as_bytes().to_vec(), Tag::Str(v.as_bytes().to_vec()));
    Tag::Compound(vec![s("ip", ip), s("name", name), (b"hidden".to_vec(), Tag::Raw(1, vec![0]))])
}

/// Contenu à écrire, ou `None` s'il y a déjà une entrée de même ip ou de même nom. `data` absent : fichier à créer.
/// Les autres entrées (et clés racine) sont réencodées à l'identique.
pub fn with_server(data: Option<&[u8]>, name: &str, ip: &str) -> Result<Option<Vec<u8>>> {
    let mut root = match data {
        Some(d) => decode(d)?,
        None => Tag::Compound(vec![]),
    };
    let Tag::Compound(keys) = &mut root else { unreachable!() };
    if !keys.iter().any(|(k, _)| k == b"servers") {
        keys.push((b"servers".to_vec(), Tag::List(10, vec![])));
    }
    let Some((_, list)) = keys.iter_mut().find(|(k, _)| k == b"servers") else { unreachable!() };
    let Tag::List(t, items) = list else { bail!("servers.dat : « servers » n'est pas une liste") };
    if items.is_empty() {
        *t = 10;
    } else if *t != 10 {
        bail!("servers.dat : liste inattendue");
    }
    if items.iter().any(|e| e.str_of("ip").is_some_and(|v| v.eq_ignore_ascii_case(ip.as_bytes())) || e.str_of("name") == Some(name.as_bytes())) {
        return Ok(None);
    }
    items.push(new_entry(name, ip));
    Ok(Some(encode(&root)))
}

/// Ajoute le serveur à `<instance>/servers.dat` (écriture atomique `.part` puis renommage). `Ok(false)` : déjà présent.
/// Fichier illisible : erreur, rien n'est modifié.
pub fn add_server(instance: &Path, name: &str, ip: &str) -> Result<bool> {
    let file = instance.join("servers.dat");
    let data = match std::fs::metadata(&file) {
        Ok(m) if m.len() > MAX_SIZE => bail!("servers.dat trop gros"),
        Ok(_) => Some(std::fs::read(&file)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let Some(out) = with_server(data.as_deref(), name, ip)? else { return Ok(false) };
    std::fs::create_dir_all(instance)?;
    let part = instance.join("servers.dat.part");
    std::fs::write(&part, out)?;
    std::fs::rename(&part, &file)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(d: &[u8]) -> Vec<Tag> {
        let Tag::Compound(k) = decode(d).unwrap() else { panic!() };
        let Some((_, Tag::List(_, v))) = k.into_iter().find(|(k, _)| k == b"servers") else { panic!() };
        v
    }

    fn user_file() -> Vec<u8> {
        // Entrée d'utilisateur avec icône, acceptTextures (byte), clés exotiques (long, tableau d'entiers) et nom non ASCII.
        let e = Tag::Compound(vec![
            (b"ip".to_vec(), Tag::Str(b"play.exemple.fr:25999".to_vec())),
            (b"name".to_vec(), Tag::Str("Mon serveur é".as_bytes().to_vec())),
            (b"icon".to_vec(), Tag::Str(b"iVBORw0KGgo=".to_vec())),
            (b"acceptTextures".to_vec(), Tag::Raw(1, vec![1])),
            (b"exotic".to_vec(), Tag::Raw(4, 7i64.to_be_bytes().to_vec())),
            (b"ints".to_vec(), Tag::Raw(11, [&2i32.to_be_bytes()[..], &1i32.to_be_bytes(), &2i32.to_be_bytes()].concat())),
            (b"hidden".to_vec(), Tag::Raw(1, vec![0])),
        ]);
        encode(&Tag::Compound(vec![(b"servers".to_vec(), Tag::List(10, vec![e])), (b"autre".to_vec(), Tag::Raw(3, 5i32.to_be_bytes().to_vec()))]))
    }

    #[test]
    fn creates_file_when_absent_and_round_trips() {
        let out = with_server(None, "Classroom Run", "home-server.x.ts.net").unwrap().unwrap();
        assert_eq!(&out[..3], [10, 0, 0]);
        let e = entries(&out);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].str_of("name"), Some(&b"Classroom Run"[..]));
        assert_eq!(e[0].str_of("ip"), Some(&b"home-server.x.ts.net"[..]));
        assert_eq!(encode(&decode(&out).unwrap()), out);
    }

    #[test]
    fn keeps_existing_entries_byte_for_byte() {
        let before = user_file();
        let out = with_server(Some(&before), "Classroom Run", "mon.minestrator.com").unwrap().unwrap();
        let (old, new) = (entries(&before), entries(&out));
        assert_eq!((old.len(), new.len()), (1, 2));
        let (mut a, mut b) = (vec![], vec![]);
        old[0].write(&mut a);
        new[0].write(&mut b);
        assert_eq!(a, b);
        assert!(out.windows(a.len()).any(|w| w == a.as_slice()));
        assert_eq!(new[1].str_of("ip"), Some(&b"mon.minestrator.com"[..]));
        assert!(out.windows(5).any(|w| w == b"autre"), "clé racine étrangère conservée");
    }

    #[test]
    fn no_duplicate_by_ip_or_name() {
        let before = user_file();
        assert!(with_server(Some(&before), "Autre", "PLAY.exemple.fr:25999").unwrap().is_none());
        assert!(with_server(Some(&before), "Mon serveur é", "x.y").unwrap().is_none());
        let once = with_server(Some(&before), "Pack", "a.b").unwrap().unwrap();
        assert!(with_server(Some(&once), "Pack", "a.b").unwrap().is_none());
    }

    #[test]
    fn unreadable_is_an_error() {
        for bad in [&b""[..], b"\x1f\x8b\x08gzip", b"\x0a\x00\x00\x01", b"\x0a\x00\x00\x00\x00extra"] {
            assert!(with_server(Some(bad), "n", "i").is_err());
        }
        let f = encode(&Tag::Compound(vec![(b"servers".to_vec(), Tag::Str(b"x".to_vec()))]));
        assert!(with_server(Some(&f), "n", "i").is_err());
    }

    #[test]
    fn empty_list_of_unknown_type_is_fixed() {
        let f = encode(&Tag::Compound(vec![(b"servers".to_vec(), Tag::List(0, vec![]))]));
        assert_eq!(entries(&with_server(Some(&f), "n", "i").unwrap().unwrap()).len(), 1);
    }

    #[test]
    fn add_server_writes_atomically_and_leaves_bad_file_alone() {
        let d = std::env::temp_dir().join(format!("opaline-servers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(add_server(&d, "P", "h.example").unwrap());
        assert!(!d.join("servers.dat.part").exists());
        assert!(!add_server(&d, "P", "h.example").unwrap());
        std::fs::write(d.join("servers.dat"), b"garbage").unwrap();
        assert!(add_server(&d, "P", "h.example").is_err());
        assert_eq!(std::fs::read(d.join("servers.dat")).unwrap(), b"garbage");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
