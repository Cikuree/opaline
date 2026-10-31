#!/usr/bin/env python3
"""Outils packwiz sans binaire packwiz.

  pack-tools.py index [--pack pack]
      Regenere index.toml (tous les *.pw.toml de mods/ et shaderpacks/) puis le hash d'index de pack.toml.
  pack-tools.py add-jar --jar J --version X.Y.Z [--pack pack]
      Ecrit mods/classroomrun.pw.toml (side both, url = asset de release du depot public, sha512 du jar), puis reindexe.
Hashes sha256 sur les octets exacts (pack/ doit rester en LF).
"""
import argparse, hashlib, re, sys, tomllib
from pathlib import Path

DIST = "https://github.com/Cikuree/classroom-run-dist/releases/download"


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


def add_jar(pack, jar, version):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        sys.exit(f"version invalide: {version}")
    name = f"classroomrun-{version}.jar"
    h = hashlib.sha512(jar.read_bytes()).hexdigest()
    body = (f'name = "Classroom Run"\nfilename = "{name}"\nside = "both"\n\n[download]\n'
            f'url = "{DIST}/v{version}/{name}"\nhash-format = "sha512"\nhash = "{h}"\n')
    (pack / "mods" / "classroomrun.pw.toml").write_bytes(body.encode())


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["index", "add-jar"])
    ap.add_argument("--pack", type=Path, default=Path(__file__).resolve().parent.parent / "pack")
    ap.add_argument("--jar", type=Path)
    ap.add_argument("--version")
    a = ap.parse_args()
    if a.cmd == "add-jar":
        if not (a.jar and a.jar.is_file() and a.version):
            sys.exit("--jar (existant) et --version requis")
        add_jar(a.pack, a.jar, a.version)
    reindex(a.pack)
