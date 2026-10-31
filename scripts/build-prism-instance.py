#!/usr/bin/env python3
"""Construit classroom-run-prism.zip (instance Prism Launcher importable).

Usage: build-prism-instance.py --pack packs/classroom-run --jar build/libs/classroomrun-0.1.0.jar [--out dist/classroom-run-prism.zip]
Mods et shaderpacks client/both lus dans packs/<id>/{mods,shaderpacks}/*.pw.toml, telecharges et verifies (hash du manifeste).
"""
import argparse, hashlib, json, sys, tomllib, urllib.request, zipfile
from pathlib import Path

import importlib.util
_spec = importlib.util.spec_from_file_location("pack_tools", Path(__file__).resolve().parent / "pack-tools.py")
_tools = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_tools)
raw_files = _tools.raw_files

ROOT = Path(__file__).resolve().parent.parent
SIDES = {"client", "both"}


def fetch(url, algo, expected):
    req = urllib.request.Request(url, headers={"User-Agent": "classroom-run-release"})
    data = urllib.request.urlopen(req, timeout=120).read()
    got = hashlib.new(algo, data).hexdigest()
    if got != expected:
        sys.exit(f"hash {algo} invalide pour {url}: {got} != {expected}")
    return data


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--jar", required=True, type=Path)
    ap.add_argument("--pack", type=Path, default=ROOT / "packs" / "classroom-run")
    ap.add_argument("--out", type=Path, default=ROOT / "dist" / "classroom-run-prism.zip")
    a = ap.parse_args()
    if not a.jar.is_file():
        sys.exit(f"jar introuvable: {a.jar}")

    pack_dir = a.pack
    pack = tomllib.loads((a.pack / "pack.toml").read_text("utf-8"))
    mc, loader = pack["versions"]["minecraft"], pack["versions"]["fabric"]
    name = pack["name"]

    # Composants Prism pour 26.3 (non obfusque) : verifies sur meta.prismlauncher.org
    # (fabric-loader requiert net.fabricmc.intermediary, qui requiert net.minecraft == mc).
    mmc = {"formatVersion": 1, "components": [
        {"uid": "net.minecraft", "version": mc, "important": True},
        {"uid": "net.fabricmc.intermediary", "version": mc, "dependencyOnly": True},
        {"uid": "net.fabricmc.fabric-loader", "version": loader},
    ]}
    cfg = f"InstanceType=OneSix\nname={name}\nJoinServerOnLaunch=false\n"

    a.out.parent.mkdir(parents=True, exist_ok=True)
    tmp = a.out.with_name(a.out.name + ".part")
    with zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("instance.cfg", cfg)
        z.writestr("mmc-pack.json", json.dumps(mmc, indent=4))
        for rel in _tools.pack_files(pack_dir)[0]:  # tous les *.pw.toml (mods, shaderpacks, resourcepacks...)
            sub = rel.rpartition("/")[0]
            m = tomllib.loads((pack_dir / rel).read_text("utf-8"))
            # le jar passe par --jar (l'URL publique n'existe pas encore pendant la CI)
            if m.get("side", "both") not in SIDES or m["filename"] == a.jar.name:
                continue
            d = m["download"]
            z.writestr(f".minecraft/{sub}/{m['filename']}", fetch(d["url"], d["hash-format"], d["hash"]))
            print(f"{sub}:", m["filename"])
        z.write(a.jar, f".minecraft/mods/{a.jar.name}")
        # fichiers bruts du pack (configs, options.txt...), memes regles que l'index
        for f in raw_files(pack_dir):
            z.write(f, f".minecraft/{f.relative_to(pack_dir).as_posix()}")
        print("mod:", a.jar.name)
    tmp.replace(a.out)
    print("ecrit:", a.out)


if __name__ == "__main__":
    main()
