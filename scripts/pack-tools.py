#!/usr/bin/env python3
"""Outils packwiz sans binaire packwiz.

  pack-tools.py index [--pack packs/classroom-run]
      Regenere index.toml (tous les *.pw.toml de mods/ et shaderpacks/) puis le hash d'index de pack.toml.
  pack-tools.py add-jar --jar J --version X.Y.Z --repo OWNER/REPO [--pack packs/classroom-run]
      Ecrit mods/classroomrun.pw.toml (side both, url = asset de release du depot --repo, sha512 du jar), puis reindexe.
  pack-tools.py catalog --tag vX.Y.Z --repo OWNER/REPO [--out packs.json]
      Genere packs.json (contrat Opaline §3) depuis tous les packs/<id>/ ; valide chaque opaline.toml.
Hashes sha256 sur les octets exacts (packs/ doit rester en LF).
"""
import argparse, hashlib, json, re, sys, tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def sha256(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def reindex(pack):
    out = ['hash-format = "sha256"', ""]
    for d in ("mods", "shaderpacks"):
        for f in sorted((pack / d).glob("*.pw.toml")):
            out += ["[[files]]", f'file = "{d}/{f.name}"', f'hash = "{sha256(f)}"', "metafile = true", ""]
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
    s = o.get("server")
    if not isinstance(s, dict):
        bad("section [server] requise")
    if not isinstance(s.get("host"), str) or not re.fullmatch(r"[a-z0-9-]+", s["host"]):
        bad("`server.host` requis, de la forme [a-z0-9-]+ (nom MagicDNS court)")
    if not isinstance(s.get("motd"), str) or not s["motd"]:
        bad("`server.motd` requis (texte non vide)")
    for k in ("port", "statsPort"):
        v = s.get(k)
        if type(v) is not int or not 0 <= v <= 65535 or (k == "port" and v == 0):
            bad(f"`server.{k}` requis, entier 0-65535" + (" (0 = pas d'endpoint)" if k == "statsPort" else ", > 0"))
    return o


def catalog(root, tag, repo, out):
    if not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
        sys.exit(f"tag invalide: {tag}")
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        sys.exit(f"depot invalide: {repo}")
    packs = []
    for d in sorted(p for p in root.iterdir() if p.is_dir()):
        if not (d / "pack.toml").is_file():
            sys.exit(f"{d.name}: pack.toml manquant")
        o, pt = load_opaline(d), tomllib.loads((d / "pack.toml").read_text("utf-8"))
        v = pt["versions"]
        loaders = [k for k in v if k != "minecraft"]
        if len(loaders) != 1:
            sys.exit(f"{d.name}/pack.toml: un seul loader attendu dans [versions], trouve {loaders}")
        packs.append({
            "id": d.name, "name": o["name"], "description": o["description"],
            "mc": v["minecraft"], "loader": loaders[0], "loaderVersion": v[loaders[0]],
            "modsCount": len(list((d / "mods").glob("*.pw.toml"))),
            "thumbnail": o["thumbnail"], "widgets": o["widgets"],
            "pack": f"https://github.com/{repo}/releases/download/{tag}/pack-{d.name}.zip",
            "packHash": pt["index"]["hash"], "server": o["server"],
        })
    out.write_text(json.dumps({"packs": packs}, ensure_ascii=False, indent=2) + "\n", "utf-8")
    print("catalogue:", out, [p["id"] for p in packs])


def add_jar(pack, jar, version, repo):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        sys.exit(f"version invalide: {version}")
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        sys.exit(f"depot invalide: {repo}")
    name = f"classroomrun-{version}.jar"
    h = hashlib.sha512(jar.read_bytes()).hexdigest()
    body = (f'name = "Classroom Run"\nfilename = "{name}"\nside = "both"\n\n[download]\n'
            f'url = "https://github.com/{repo}/releases/download/v{version}/{name}"\nhash-format = "sha512"\nhash = "{h}"\n')
    (pack / "mods" / "classroomrun.pw.toml").write_bytes(body.encode())


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["index", "add-jar", "catalog"])
    ap.add_argument("--pack", type=Path, default=ROOT / "packs" / "classroom-run")
    ap.add_argument("--tag")
    ap.add_argument("--out", type=Path, default=Path("packs.json"))
    ap.add_argument("--jar", type=Path)
    ap.add_argument("--version")
    ap.add_argument("--repo", help="OWNER/REPO des releases (ex. $GITHUB_REPOSITORY)")
    a = ap.parse_args()
    if a.cmd == "catalog":
        if not (a.tag and a.repo):
            sys.exit("--tag et --repo requis")
        catalog(ROOT / "packs", a.tag, a.repo, a.out)
        sys.exit(0)
    if a.cmd == "add-jar":
        if not (a.jar and a.jar.is_file() and a.version and a.repo):
            sys.exit("--jar (existant), --version et --repo requis")
        add_jar(a.pack, a.jar, a.version, a.repo)
    reindex(a.pack)
