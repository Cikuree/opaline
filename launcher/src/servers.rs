//! Liste Multijoueur de l'instance (`servers.dat`, NBT non compressé comme `NbtIo.write` de Minecraft) : ajout (ou mise à
//! jour de l'adresse) de l'entrée du pack sans jamais toucher aux autres. Lecteur/écrivain NBT minimal mais sans perte.
use anyhow::{Result, bail};
use std::io::Write;
use std::path::Path;

const MAX_SIZE: u64 = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 1_000_000;
const DEFAULT_PORT_SUFFIX: &[u8] = b":25565";

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    /// Byte, short, int, long, float, double et tableaux : charge utile brute (longueur incluse pour les tableaux).
    Raw(u8, Vec<u8>),
    /// Chaîne en « Modified UTF-8 » (octets conservés tels quels).
    Str(Vec<u8>),
    /// Liste d'éléments non primitifs (ou vide).
    List(u8, Vec<Tag>),
    /// Liste de primitives (types 1 à 6), gardée en brut : éléments concaténés.
    PrimList(u8, Vec<u8>),
    Compound(Vec<(Vec<u8>, Tag)>),
}

struct Reader<'a> {
    data: &'a [u8],
    nodes: usize,
}

const PRIM_SIZE: [usize; 6] = [1, 2, 4, 8, 4, 8];

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.data.len() {
            bail!("NBT tronqué");
        }
        let (a, b) = self.data.split_at(n);
        self.data = b;
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
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            bail!("NBT trop volumineux");
        }
        Ok(match id {
            1..=6 => Tag::Raw(id, self.take(PRIM_SIZE[id as usize - 1])?.to_vec()),
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
                if n == 0 {
                    return Ok(Tag::List(t, vec![]));
                }
                if t <= 6 {
                    return Ok(Tag::PrimList(t, self.take(n.saturating_mul(PRIM_SIZE[t as usize - 1]))?.to_vec()));
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
            Tag::List(..) | Tag::PrimList(..) => 9,
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
            Tag::PrimList(t, b) => {
                out.push(*t);
                out.extend_from_slice(&((b.len() / PRIM_SIZE[*t as usize - 1]) as i32).to_be_bytes());
                out.extend_from_slice(b);
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
}

fn write_str(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s);
}

/// « Modified UTF-8 » de Java (`DataOutput.writeUTF`) : NUL en `C0 80`, hors BMP en paire de substitution (3 octets chacun).
/// `None` si le résultat dépasse 65535 octets (longueur non représentable en NBT).
pub fn modified_utf8(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len());
    for u in s.encode_utf16() {
        match u {
            0 => out.extend_from_slice(&[0xC0, 0x80]),
            1..=0x7F => out.push(u as u8),
            0x80..=0x7FF => out.extend_from_slice(&[0xC0 | (u >> 6) as u8, 0x80 | (u & 0x3F) as u8]),
            _ => out.extend_from_slice(&[0xE0 | (u >> 12) as u8, 0x80 | ((u >> 6) & 0x3F) as u8, 0x80 | (u & 0x3F) as u8]),
        }
    }
    (out.len() <= u16::MAX as usize).then_some(out)
}

/// Fichier NBT : compound racine (nom ignoré) ; toute anomalie (dont octets en trop) est une erreur.
pub fn decode(bytes: &[u8]) -> Result<Tag> {
    let mut r = Reader { data: bytes, nodes: 0 };
    if r.u8()? != 10 {
        bail!("racine NBT inattendue");
    }
    r.string()?;
    let t = r.payload(10, 0)?;
    if !r.data.is_empty() {
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

fn str_of<'a>(entry: &'a Tag, key: &str) -> Option<&'a [u8]> {
    let Tag::Compound(v) = entry else { return None };
    v.iter().find_map(|(k, x)| match x {
        Tag::Str(s) if k == key.as_bytes() => Some(s.as_slice()),
        _ => None,
    })
}

/// Adresse comparable : sans espaces, en minuscules, sans `:25565` final (port par défaut).
fn norm_ip(ip: &[u8]) -> Vec<u8> {
    let mut v = ip.trim_ascii().to_ascii_lowercase();
    if let Some(h) = v.strip_suffix(DEFAULT_PORT_SUFFIX) {
        // `a:b:c:25565` (IPv6 sans crochets) reste ambigu : on ne retire que pour un hôte sans autre `:` ou entre crochets.
        if !h.contains(&b':') || h.ends_with(b"]") {
            v.truncate(h.len());
        }
    }
    v
}

/// Contenu à écrire, ou `None` s'il n'y a rien à changer. `data` absent : fichier à créer.
/// - une entrée a déjà cette adresse : rien ;
/// - sinon une entrée porte le nom du pack : seule son `ip` est mise à jour ;
/// - sinon une entrée est ajoutée.
///
/// Les autres entrées (et clés racine) sont réencodées à l'identique. Nom ou adresse trop longs : erreur.
pub fn with_server(data: Option<&[u8]>, name: &str, ip: &str) -> Result<Option<Vec<u8>>> {
    let (Some(name), Some(ip)) = (modified_utf8(name), modified_utf8(ip)) else { bail!("nom ou adresse trop long pour le NBT") };
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
    let want = norm_ip(&ip);
    if items.iter().any(|e| str_of(e, "ip").is_some_and(|v| norm_ip(v) == want)) {
        return Ok(None);
    }
    if let Some(Tag::Compound(fields)) = items.iter_mut().find(|e| str_of(e, "name") == Some(name.as_slice())) {
        match fields.iter_mut().find(|(k, _)| k == b"ip") {
            Some((_, v)) => *v = Tag::Str(ip),
            None => fields.push((b"ip".to_vec(), Tag::Str(ip))),
        }
    } else {
        let s = |k: &str, v: Vec<u8>| (k.as_bytes().to_vec(), Tag::Str(v));
        items.push(Tag::Compound(vec![s("ip", ip), s("name", name), (b"hidden".to_vec(), Tag::Raw(1, vec![0]))]));
    }
    Ok(Some(encode(&root)))
}

fn write_atomic(part: &Path, file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = std::fs::File::create(part)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(part, file)
}

/// Ajoute (ou met à jour) le serveur dans `<instance>/servers.dat` (écriture atomique `.part` puis renommage).
/// `Ok(true)` : l'entrée est présente à l'issue ; `Ok(false)` : nom ou adresse trop long, rien n'est écrit (message sans adresse).
/// Fichier illisible : erreur, rien n'est modifié.
pub fn add_server(instance: &Path, name: &str, ip: &str) -> Result<bool> {
    let file = instance.join("servers.dat");
    let data = match std::fs::metadata(&file) {
        Ok(m) if m.len() > MAX_SIZE => bail!("servers.dat trop gros"),
        Ok(_) => Some(std::fs::read(&file)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    if modified_utf8(name).is_none() || modified_utf8(ip).is_none() {
        eprintln!("servers.dat : nom ou adresse trop long, liste Multijoueur non modifiée");
        return Ok(false);
    }
    let Some(out) = with_server(data.as_deref(), name, ip)? else { return Ok(true) };
    std::fs::create_dir_all(instance)?;
    let part = instance.join("servers.dat.part");
    write_atomic(&part, &file, &out).inspect_err(|_| {
        let _ = std::fs::remove_file(&part);
    })?;
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

    fn s(v: &str) -> Tag {
        Tag::Str(modified_utf8(v).unwrap())
    }

    fn user_file() -> Vec<u8> {
        // Entrée d'utilisateur avec icône, acceptTextures (byte), clés exotiques (long, tableau d'entiers) et nom non ASCII.
        let e = Tag::Compound(vec![
            (b"ip".to_vec(), s("play.exemple.fr:25999")),
            (b"name".to_vec(), s("Mon serveur é")),
            (b"icon".to_vec(), s("iVBORw0KGgo=")),
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
        assert_eq!(str_of(&e[0], "name"), Some(&b"Classroom Run"[..]));
        assert_eq!(str_of(&e[0], "ip"), Some(&b"home-server.x.ts.net"[..]));
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
        assert_eq!(str_of(&new[1], "ip"), Some(&b"mon.minestrator.com"[..]));
        assert!(out.windows(5).any(|w| w == b"autre"), "clé racine étrangère conservée");
    }

    #[test]
    fn no_duplicate_by_ip_even_with_default_port_or_case() {
        let before = user_file();
        assert!(with_server(Some(&before), "Autre", "PLAY.exemple.fr:25999").unwrap().is_none());
        let once = with_server(Some(&before), "Pack", "a.b").unwrap().unwrap();
        assert!(with_server(Some(&once), "Pack", "a.b").unwrap().is_none());
        assert!(with_server(Some(&once), "Autre", "A.B:25565").unwrap().is_none());
        let with_port = with_server(None, "P", "h.example:25565").unwrap().unwrap();
        assert!(with_server(Some(&with_port), "P", "h.example").unwrap().is_none());
        // `::1:25565` : ambigu, non normalisé
        assert_eq!(norm_ip(b"::1:25565"), b"::1:25565");
        assert_eq!(norm_ip(b"[::1]:25565"), b"[::1]");
    }

    #[test]
    fn same_name_other_address_updates_only_that_ip() {
        let before = user_file();
        let v1 = with_server(Some(&before), "Pack", "ancien.example").unwrap().unwrap();
        let v2 = with_server(Some(&v1), "Pack", "nouveau.example:25999").unwrap().unwrap();
        let (a, b) = (entries(&v1), entries(&v2));
        assert_eq!(b.len(), 2);
        assert_eq!(a[0], b[0], "entrée utilisateur intacte");
        assert_eq!(str_of(&b[1], "ip"), Some(&b"nouveau.example:25999"[..]));
        assert_eq!(str_of(&b[1], "name"), Some(&b"Pack"[..]));
        // le nom d'une entrée utilisateur sans cette adresse est aussi mis à jour (c'est son ip seule)
        let c = entries(&with_server(Some(&before), "Mon serveur é", "x.y").unwrap().unwrap());
        assert_eq!((c.len(), str_of(&c[0], "ip")), (1, Some(&b"x.y"[..])));
        assert_eq!(str_of(&c[0], "icon"), Some(&b"iVBORw0KGgo="[..]));
    }

    #[test]
    fn modified_utf8_matches_java() {
        assert_eq!(modified_utf8("a\0é").unwrap(), [b'a', 0xC0, 0x80, 0xC3, 0xA9]);
        // U+1F600 = paire de substitution D83D DE00, chacune sur 3 octets
        assert_eq!(modified_utf8("\u{1F600}").unwrap(), [0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        assert!(modified_utf8(&"é".repeat(32767)).is_some() && modified_utf8(&"é".repeat(32768)).is_none());
        assert!(modified_utf8(&"a".repeat(65536)).is_none());
        let out = with_server(None, "Pack \u{1F600}\0", "h.example").unwrap().unwrap();
        assert_eq!(str_of(&entries(&out)[0], "name"), Some(&[b"Pack ".as_slice(), &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80, 0xC0, 0x80]].concat()[..]));
        assert!(with_server(None, &"a".repeat(70000), "h").is_err());
    }

    #[test]
    fn unreadable_is_an_error() {
        for bad in [&b""[..], b"\x1f\x8b\x08gzip", b"\x0a\x00\x00\x01", b"\x0a\x00\x00\x00\x00extra"] {
            assert!(with_server(Some(bad), "n", "i").is_err());
        }
        let f = encode(&Tag::Compound(vec![(b"servers".to_vec(), s("x"))]));
        assert!(with_server(Some(&f), "n", "i").is_err());
    }

    #[test]
    fn empty_list_of_unknown_type_is_fixed() {
        let f = encode(&Tag::Compound(vec![(b"servers".to_vec(), Tag::List(0, vec![]))]));
        assert_eq!(entries(&with_server(Some(&f), "n", "i").unwrap().unwrap()).len(), 1);
    }

    #[test]
    fn nested_and_exotic_tags_round_trip() {
        let inner = Tag::List(9, vec![Tag::PrimList(3, [1i32.to_be_bytes(), 2i32.to_be_bytes()].concat()), Tag::List(0, vec![])]);
        let longs = Tag::Raw(12, [&2i32.to_be_bytes()[..], &9i64.to_be_bytes(), &8i64.to_be_bytes()].concat());
        let f = encode(&Tag::Compound(vec![
            (b"servers".to_vec(), Tag::List(10, vec![])),
            (b"ll".to_vec(), Tag::List(9, vec![inner.clone(), inner])),
            (b"longs".to_vec(), longs),
            (b"shorts".to_vec(), Tag::PrimList(2, vec![0, 1, 0, 2])),
        ]));
        assert_eq!(encode(&decode(&f).unwrap()), f);
        let out = with_server(Some(&f), "P", "h.example").unwrap().unwrap();
        let Tag::Compound(k) = decode(&out).unwrap() else { panic!() };
        assert!(k.iter().any(|(n, _)| n == b"ll") && k.iter().any(|(n, _)| n == b"longs"));
    }

    fn nested(levels: usize) -> Vec<u8> {
        // racine compound -> `levels` compounds imbriqués
        let mut v = vec![10, 0, 0];
        for _ in 0..levels {
            v.extend_from_slice(&[10, 0, 1, b'a']);
        }
        v.extend(std::iter::repeat_n(0u8, levels + 1));
        v
    }

    #[test]
    fn depth_and_size_limits() {
        assert!(decode(&nested(60)).is_ok());
        assert!(decode(&nested(70)).is_err());
        // liste de 2 milliards de listes déclarée sur un fichier court : rejet rapide (tronqué), sans allocation géante
        let mut huge = vec![10, 0, 0, 9, 0, 1, b'l', 9];
        huge.extend_from_slice(&i32::MAX.to_be_bytes());
        assert!(decode(&huge).is_err());
        let mut huge_prim = vec![10, 0, 0, 9, 0, 1, b'l', 4];
        huge_prim.extend_from_slice(&i32::MAX.to_be_bytes());
        assert!(decode(&huge_prim).is_err());
        // liste énorme de primitives : stockée en brut, acceptée
        let n = 3_000_000usize;
        let mut big = vec![10, 0, 0, 9, 0, 1, b'l', 1];
        big.extend_from_slice(&(n as i32).to_be_bytes());
        big.extend(std::iter::repeat_n(7u8, n));
        big.push(0);
        assert_eq!(encode(&decode(&big).unwrap()), big);
        // plus de MAX_NODES éléments non primitifs (listes vides imbriquées) : refusé
        let mut many = vec![10, 0, 0, 9, 0, 1, b'l', 9];
        many.extend_from_slice(&((MAX_NODES as i32) + 10).to_be_bytes());
        for _ in 0..(MAX_NODES + 10) {
            many.extend_from_slice(&[0, 0, 0, 0, 0]);
        }
        many.push(0);
        assert!(decode(&many).is_err());
    }

    #[test]
    fn add_server_writes_atomically_and_leaves_bad_file_alone() {
        let d = std::env::temp_dir().join(format!("opaline-servers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(add_server(&d, "P", "h.example").unwrap());
        assert!(!d.join("servers.dat.part").exists());
        let first = std::fs::read(d.join("servers.dat")).unwrap();
        assert!(add_server(&d, "P", "h.example").unwrap());
        assert_eq!(std::fs::read(d.join("servers.dat")).unwrap(), first, "déjà présent : fichier inchangé");
        // trop long : rien d'écrit
        assert!(!add_server(&d, &"x".repeat(70000), "h.example").unwrap());
        assert_eq!(std::fs::read(d.join("servers.dat")).unwrap(), first);
        std::fs::write(d.join("servers.dat"), b"garbage").unwrap();
        assert!(add_server(&d, "P", "h.example").is_err());
        assert_eq!(std::fs::read(d.join("servers.dat")).unwrap(), b"garbage");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
