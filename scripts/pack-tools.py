#!/usr/bin/env python3
"""Outils packwiz sans binaire packwiz.

  pack-tools.py index [--pack packs/classroom-run]
      Regenere index.toml (tous les *.pw.toml de mods/ et shaderpacks/) puis le hash d'index de pack.toml.
  pack-tools.py add-jar --jar J --version X.Y.Z --base-url https://github.com/OWNER/REPO/releases/download/packs/ [--pack packs/classroom-run]
      Ecrit mods/classroomrun.pw.toml (side both, url = --base-url + classroomrun-X.Y.Z.jar, sha512 du jar), puis reindexe.
  pack-tools.py catalog --base-url https://github.com/OWNER/REPO/releases/download/packs/ [--out packs.json]
      Genere packs.json (contrat Opaline §3) depuis tous les packs/<id>/ ; valide chaque opaline.toml.
  [server] map = "<url http(s)>" facultatif : carte du serveur (BlueMap...) ; validee (http/https, hote sans identifiants, 300 caracteres
maximum, ASCII visible) et exposee dans packs.json (server.map).
  [overrides] facultatif d'opaline.toml : archive des fichiers non commites (resourcepacks, datapacks...) publiee a part ;
catalog valide url / sha256 / size et les expose dans packs.json. packHash = hash d'index sans [overrides] (inchange) ;
avec : sha256_hex(utf8("<hash d'index>" + LF + "<sha256 de l'archive>")) (minuscules, LF = un saut de ligne).
Fichiers bruts (configs, options.txt, mods hors Modrinth...) : tout fichier de packs/<id>/ hors pack.toml, index.toml,
opaline.toml, *.pw.toml et fichiers caches est indexe (metafile = false, sha256).
  pack-tools.py zip --pack packs/<id> --out pack-<id>.zip
      Zip publie : pack/ avec pack.toml, index.toml, opaline.toml et les fichiers de l'index seulement.
Hashes sha256 sur les octets exacts (packs/ doit rester en LF).
"""
import argparse, hashlib, io, json, re, sys, tomllib, zipfile
from urllib.parse import urlsplit
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def sha256(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


RESERVED = {"pack.toml", "index.toml", "opaline.toml"}  # a la racine du pack seulement
# Cles de [versions] de pack.toml reconnues comme loader (core/pack.go de packwiz : fabric, quilt, forge, neoforge).
LOADERS = ("fabric", "quilt", "neoforge", "forge")
MAX_ZIP_BYTES, MAX_ENTRIES = 50 * 1024 * 1024, 5000  # launcher/src/packwiz.rs PACK_ZIP_MAX_BYTES, PACK_MAX_ENTRIES


def pack_files(pack):
    """(fichiers metafichiers .pw.toml, fichiers bruts), chemins relatifs POSIX tries ; fichiers caches ignores partout."""
    meta, raw = [], []
    for f in sorted(pack.rglob("*")):
        rel = f.relative_to(pack).as_posix()
        if not f.is_file() or any(x.startswith(".") for x in rel.split("/")) or rel in RESERVED:
            continue
        (meta if rel.endswith(".pw.toml") else raw).append(rel)
    return sorted(meta), sorted(raw)


def raw_files(pack):
    return [pack / r for r in pack_files(pack)[1]]


def check_id(pack):
    if not re.fullmatch(r"[a-z0-9-]+", pack.resolve().name):
        sys.exit(f"id de pack invalide `{pack.resolve().name}` : [a-z0-9-]+ attendu")


def make_zip(pack):
    """Octets de pack-<id>.zip : dossier pack/ contenant pack.toml, index.toml, opaline.toml et les fichiers de l'index."""
    it = tomllib.loads((pack / "index.toml").read_text("utf-8"))
    names = ["pack.toml", "index.toml", "opaline.toml"] + [e["file"] for e in it.get("files", [])]
    if len(names) > MAX_ENTRIES:
        sys.exit(f"{pack.name}: {len(names)} fichiers, limite {MAX_ENTRIES}")
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for n in names:
            z.write(pack / n, "pack/" + n)
    if buf.tell() > MAX_ZIP_BYTES:
        sys.exit(f"{pack.name}: zip de {buf.tell() >> 20} Mo, limite {MAX_ZIP_BYTES >> 20} Mo")
    return buf.getvalue()


def reindex(pack):
    check_id(pack)
    meta, raw = pack_files(pack)
    out = ['hash-format = "sha256"', ""]
    for r in meta:
        out += ["[[files]]", f'file = "{r}"', f'hash = "{sha256(pack / r)}"', "metafile = true", ""]
    for r in raw:
        out += ["[[files]]", f'file = "{r}"', f'hash = "{sha256(pack / r)}"', ""]
    idx = pack / "index.toml"
    idx.write_bytes("\n".join(out).encode())
    pt = pack / "pack.toml"
    s = pt.read_text("utf-8")
    s, n = re.subn(r'(\[index\][^\[]*?hash = ")[0-9a-f]+(")', lambda m: m[1] + sha256(idx) + m[2], s, flags=re.S)
    if n != 1:
        sys.exit("hash d'index introuvable dans pack.toml")
    pt.write_bytes(s.encode())
    tomllib.loads(s), tomllib.loads(idx.read_text("utf-8"))  # validation
    print("index:", idx, sha256(idx))


WIDGETS = {"classroomRun", "server", "news"}


def load_opaline(pack):
    """Lit et valide packs/<id>/opaline.toml ; quitte avec un message clair."""
    f = pack / "opaline.toml"
    if not f.is_file():
        sys.exit(f"{pack.name}: opaline.toml manquant")
    o = tomllib.loads(f.read_text("utf-8"))

    def bad(msg):
        sys.exit(f"{pack.name}/opaline.toml: {msg}")

    for k in ("name", "description", "thumbnail"):
        if not isinstance(o.get(k), str) or not o[k].strip():
            bad(f"champ `{k}` requis (texte non vide)")
    if not re.fullmatch(r"[a-z0-9_]+", o["thumbnail"]):
        bad("`thumbnail` doit respecter [a-z0-9_]+ (nom de texture item/ ou block/)")
    w = o.get("widgets")
    if not isinstance(w, list) or not all(isinstance(x, str) for x in w):
        bad("`widgets` requis (liste de textes)")
    if set(w) - WIDGETS:
        bad(f"widgets inconnus {sorted(set(w) - WIDGETS)} ; autorises : {sorted(WIDGETS)}")
    o["overrides"] = check_overrides(o, bad)
    s = o.get("server")  # section facultative : sans elle, pas de carte serveur ni de statistiques
    if s is None:
        return o
    if not isinstance(s, dict):
        bad("[server] doit etre une section")
    if not isinstance(s.get("host"), str) or not re.fullmatch(r"[a-z0-9-]+(\.[a-z0-9-]+)*", s["host"]):
        bad("`server.host` requis : nom court [a-z0-9-]+ (Tailscale MagicDNS) ou nom complet / IP (serveur public)")
    if not isinstance(s.get("motd"), str) or not s["motd"]:
        bad("`server.motd` requis (texte non vide)")
    for k in ("port", "statsPort"):
        v = s.get(k)
        if type(v) is not int or not 0 <= v <= 65535 or (k == "port" and v == 0):
            bad(f"`server.{k}` requis, entier 0-65535" + (" (0 = pas d'endpoint)" if k == "statsPort" else ", > 0"))
    if "map" in s and not valid_map(s["map"]):
        bad(f"`server.map` doit etre une URL http(s) sans identifiants ni espaces, {MAP_MAX_LEN} caracteres au plus")
    return o


MAP_MAX_LEN = 300  # launcher/src/catalog.rs MAP_MAX_LEN


def valid_map(u):
    """Meme regle que catalog::valid_map (Rust) : http(s), hote present, aucun identifiant, ASCII visible, longueur bornee."""
    if not isinstance(u, str) or not 0 < len(u) <= MAP_MAX_LEN or not all(0x21 <= ord(c) <= 0x7E and c != "\\" for c in u):
        return False
    try:
        p = urlsplit(u)
        h = (p.hostname or "").lower()
        local = h == "localhost" or h.endswith(".localhost") or h.startswith("127.") or h == "::1"  # carte locale refusee
        return p.scheme in ("http", "https") and bool(h) and not local and p.username is None and p.password is None and "@" not in p.netloc and p.port != 0
    except ValueError:
        return False


OVERRIDES_BASE = "https://github.com/Cikuree/opaline/releases/download/packs/"
OVERRIDES_MAX_BYTES = 2 * 1024 ** 3  # launcher/src/catalog.rs OVERRIDES_MAX_BYTES


def check_overrides(o, bad):
    """Valide la section facultative [overrides] ; renvoie {url, sha256, size} ou None."""
    ov = o.get("overrides")
    if ov is None:
        return None
    if not isinstance(ov, dict) or set(ov) != {"url", "sha256", "size"}:
        bad("[overrides] doit contenir exactement url, sha256 et size")
    u, h, n = ov["url"], ov["sha256"], ov["size"]
    if not (isinstance(u, str) and u.startswith(OVERRIDES_BASE) and re.fullmatch(r"[A-Za-z0-9._-]+\.zip", u[len(OVERRIDES_BASE):]) and ".." not in u):
        bad(f"`overrides.url` doit etre {OVERRIDES_BASE}<nom simple>.zip")
    if not (isinstance(h, str) and re.fullmatch(r"[0-9a-f]{64}", h)):
        bad("`overrides.sha256` requis : 64 caracteres hexadecimaux minuscules")
    if type(n) is not int or not 0 < n <= OVERRIDES_MAX_BYTES:
        bad("`overrides.size` requis : entier > 0 et <= 2 Go (2147483648)")
    return {"url": u, "sha256": h, "size": n}


def pack_hash(index_hash, overrides):
    """packHash : hash d'index seul sans overrides (inchange), sinon sha256 de index + LF + sha256 de l'archive."""
    if overrides is None:
        return index_hash
    return hashlib.sha256((index_hash.lower() + "\n" + overrides["sha256"]).encode()).hexdigest()


def verify(d, pt):
    """sha256(index.toml) == [index] hash de pack.toml, et hash de chaque fichier listé dans index.toml."""
    idx = d / "index.toml"
    if pt["index"].get("hash-format") != "sha256" or sha256(idx) != pt["index"]["hash"]:
        sys.exit(f"{d.name}: hash de index.toml different de [index] hash de pack.toml (lancer `pack-tools.py index --pack packs/{d.name}`)")
    it = tomllib.loads(idx.read_text("utf-8"))
    if it.get("hash-format") != "sha256":
        sys.exit(f"{d.name}/index.toml: hash-format sha256 requis")
    for e in it.get("files", []):
        f = d / e["file"]
        if not f.is_file() or sha256(f) != e["hash"]:
            sys.exit(f"{d.name}: fichier {e['file']} absent ou different du hash de index.toml (lancer `pack-tools.py index --pack packs/{d.name}`)")


def catalog(root, base, out):
    if not re.fullmatch(r"https://[\w.-]+/[\w./-]+", base):
        sys.exit(f"base d'URL invalide: {base}")
    base = base.rstrip("/") + "/"
    packs = []
    for d in sorted(p for p in root.iterdir() if p.is_dir()):
        check_id(d)
        if not (d / "pack.toml").is_file():
            sys.exit(f"{d.name}: pack.toml manquant")
        o, pt = load_opaline(d), tomllib.loads((d / "pack.toml").read_text("utf-8"))
        verify(d, pt)
        make_zip(d)  # limites de taille et d'entrees
        v = pt["versions"]
        unknown = [k for k in v if k != "minecraft" and k not in LOADERS]
        loaders = [k for k in v if k in LOADERS]
        bad = [x for x in [v["minecraft"], *(v[k] for k in loaders)] if not re.fullmatch(r"[0-9A-Za-z._+-]+", str(x)) or ".." in str(x)]
        if bad:
            sys.exit(f"{d.name}/pack.toml: version invalide dans [versions] : {bad}")
        if unknown or len(loaders) != 1:
            sys.exit(f"{d.name}/pack.toml: un seul loader parmi {'/'.join(LOADERS)} attendu dans [versions], trouve {loaders + unknown}")
        packs.append({
            "id": d.name, "name": o["name"], "description": o["description"],
            "mc": v["minecraft"], "loader": loaders[0], "loaderVersion": v[loaders[0]],
            "modsCount": len(list((d / "mods").glob("*.pw.toml"))),
            "thumbnail": o["thumbnail"], "widgets": o["widgets"],
            "pack": f"{base}pack-{d.name}.zip",
            "packHash": pack_hash(pt["index"]["hash"], o["overrides"]), "server": o.get("server"),
        })
        if o["overrides"]:
            packs[-1]["overrides"] = o["overrides"]
    out.write_text(json.dumps({"packs": packs}, ensure_ascii=False, indent=2) + "\n", "utf-8")
    print("catalogue:", out, [p["id"] for p in packs])


def add_jar(pack, jar, version, base_url):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        sys.exit(f"version invalide: {version}")
    if not re.fullmatch(r"https://github\.com/[\w.-]+/[\w.-]+/releases/download/[\w.-]+/", base_url):
        sys.exit(f"base d'URL invalide: {base_url}")
    name = f"classroomrun-{version}.jar"
    h = hashlib.sha512(jar.read_bytes()).hexdigest()
    body = (f'name = "Classroom Run"\nfilename = "{name}"\nside = "both"\n\n[download]\n'
            f'url = "{base_url}{name}"\nhash-format = "sha512"\nhash = "{h}"\n')
    (pack / "mods" / "classroomrun.pw.toml").write_bytes(body.encode())


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["index", "add-jar", "catalog", "zip"])
    ap.add_argument("--pack", type=Path, default=ROOT / "packs" / "classroom-run")
    ap.add_argument("--base-url")
    ap.add_argument("--out", type=Path, default=Path("packs.json"))
    ap.add_argument("--jar", type=Path)
    ap.add_argument("--version")
    a = ap.parse_args()
    if a.cmd == "catalog":
        if not a.base_url:
            sys.exit("--base-url requis")
        catalog(ROOT / "packs", a.base_url, a.out)
        sys.exit(0)
    if a.cmd == "zip":
        check_id(a.pack)
        a.out.write_bytes(make_zip(a.pack))
        sys.exit(0)
    if a.cmd == "add-jar":
        if not (a.jar and a.jar.is_file() and a.version and a.base_url):
            sys.exit("--jar (existant), --version et --base-url requis")
        add_jar(a.pack, a.jar, a.version, a.base_url)
    reindex(a.pack)
