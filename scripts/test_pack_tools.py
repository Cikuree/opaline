"""Tests sans reseau de [overrides] dans pack-tools.py : python3 -I scripts/test_pack_tools.py"""
import importlib.util, json, shutil, sys, tempfile
from pathlib import Path
repo = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("pt", repo / "scripts/pack-tools.py"); pt = importlib.util.module_from_spec(spec); spec.loader.exec_module(pt)
B = pt.OVERRIDES_BASE; H = "ab" * 32
def run(block):
    root = Path(tempfile.mkdtemp()); shutil.copytree(repo / "packs/classroom-run", root / "classroom-run")
    f = root / "classroom-run/opaline.toml"; f.write_text(f.read_text("utf-8") + "\n" + block, "utf-8")
    out = root / "o.json"
    try:
        pt.catalog(root, B, out); return json.loads(out.read_text("utf-8"))["packs"][0]
    except SystemExit as e:
        return str(e)
    finally: shutil.rmtree(root)
ok = f'[overrides]\nurl = "{B}overrides-x-0123abcd.zip"\nsha256 = "{H}"\nsize = 123456789\n'
base = run("")  # packHash sans [overrides] = hash d index
assert "overrides" not in base
good = run(ok); assert good["overrides"] == {"url": f"{B}overrides-x-0123abcd.zip", "sha256": H, "size": 123456789}, good
assert good["packHash"] != base["packHash"] and len(good["packHash"]) == 64
import hashlib; assert good["packHash"] == hashlib.sha256((base["packHash"] + "\n" + H).encode()).hexdigest()
assert run(ok.replace(H, "cd" * 32))["packHash"] != good["packHash"]
for name, bad in {
  "url hors base": ok.replace(B, "https://evil.example/"), "url http": ok.replace("https://", "http://"),
  "url sous-dossier": ok.replace("overrides-x", "a/overrides-x"), "url traversal": ok.replace("overrides-x-0123abcd.zip", "..%2f.zip"),
  "url .zip absent": ok.replace(".zip", ".exe"), "url vide nom": ok.replace("overrides-x-0123abcd.zip", ".zip"),
  "sha court": ok.replace(H, "ab" * 31), "sha majuscules": ok.replace(H, "AB" * 32), "sha non hex": ok.replace(H, "zz" * 32),
  "size 0": ok.replace("123456789", "0"), "size negatif": ok.replace("123456789", "-5"), "size > 2Go": ok.replace("123456789", str(2 * 1024**3 + 1)),
  "size texte": ok.replace("123456789", '"12"'), "size float": ok.replace("123456789", "1.5"), "champ en trop": ok + 'x = 1\n', "champ manquant": ok.replace('size = 123456789\n', ''),
}.items():
    r = run(bad); assert isinstance(r, str) and "overrides" in r, (name, r)
assert run(ok.replace("123456789", str(2 * 1024**3)))["overrides"]["size"] == 2 * 1024**3
print("ok")
# [server] map : validee, exposee dans packs.json, absente sinon
def run_map(value):
    root = Path(tempfile.mkdtemp()); shutil.copytree(repo / "packs/classroom-run", root / "classroom-run")
    f = root / "classroom-run/opaline.toml"; f.write_text(f.read_text("utf-8").rstrip("\n") + f"\nmap = {value}\n", "utf-8")
    out = root / "o.json"
    try:
        pt.catalog(root, B, out); return json.loads(out.read_text("utf-8"))["packs"][0]["server"]
    except SystemExit as e:
        return str(e)
    finally: shutil.rmtree(root)
assert "map" not in base["server"]
for good_url in ["https://map.example.fr/", "http://home-server:8100", "http://home-server:8100/#world:0:0:0", "http://91.197.6.154:8100/#world:0:0:0", "https://m.example.fr/map?x=1&y=2"]:
    assert run_map(json.dumps(good_url))["map"] == good_url, good_url
for name, bad_url in {"ftp": "ftp://x.fr/", "javascript": "javascript:alert(1)", "sans schema": "map.example.fr", "identifiants": "https://u:p@x.fr/", "user seul": "https://u@x.fr/",
  "arobase": "https://x.fr@evil.fr/", "espace": "https://x.fr/a b", "vide": "", "trop long": "https://x.fr/" + "a" * 300, "unicode": "https://x.fr/é", "sans hote": "https:///x", "file": "file:///c:/x", "port 0": "https://x.fr:0/", "localhost": "http://localhost:8100/", "sous-domaine localhost": "http://a.localhost/", "127.0.0.1": "http://127.0.0.1:8100/", "127.x": "http://127.1.2.3/", "ipv6 local": "http://[::1]:8100/", "antislash": "https://x.fr\\@evil.fr/", "un slash": "https:/x.fr/", "trois slashs": "https:///x.fr"}.items():
    r = run_map(json.dumps(bad_url)); assert isinstance(r, str) and "server.map" in r, (name, r)
r = run_map("12"); assert isinstance(r, str) and "server.map" in r, r
print("ok map")
