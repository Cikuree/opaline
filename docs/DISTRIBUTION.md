# Dépôt public de distribution

Dépôt : https://github.com/Cikuree/classroom-run-dist (public). Il ne contient **aucun code source**, seulement ce qui est distribué. Il est alimenté uniquement par la CI du dépôt privé `OddNode-design/classroom-run`, sur tag `v*`.

## Contenu de la branche `main`

```
pack/pack.toml
pack/index.toml
pack/mods/*.pw.toml          # dont classroomrun.pw.toml, qui pointe vers l'asset de release
pack/shaderpacks/*.pw.toml   # Complementary Reimagined (lien Modrinth, jamais le zip)
launcher.json                # {"version": "X.Y.Z", "windows": "<url>", "linux": "<url>"}
CREDITS.md                   # crédits des mods et du pack de shaders
README.md
```

- Le launcher lit le pack depuis `https://raw.githubusercontent.com/Cikuree/classroom-run-dist/main/pack/`.
- Les hashes sont vérifiés de bout en bout : `pack.toml` → `index.toml` → chaque `.pw.toml` → fichier téléchargé.

## Releases (tag `vX.Y.Z`, identique au tag du dépôt privé)

| Asset | Contenu |
|---|---|
| `classroomrun-X.Y.Z.jar` | le mod (référencé par `pack/mods/classroomrun.pw.toml`, avec son sha512) |
| `classroom-launcher-windows.exe` | le launcher Windows |
| `classroom-launcher-linux` | le launcher Linux |
| `classroom-run-prism.zip` | l'instance Prism importable |

## Publication

- La CI pousse dans ce dépôt avec le secret `DIST_TOKEN`.
- `DIST_TOKEN` est un jeton GitHub fine-grained du compte `Cikuree`, limité à `classroom-run-dist`, avec la permission « Contents: Read and write ».
- Le jeton est créé et enregistré par l'utilisateur, jamais écrit dans le dépôt ni dans un log.
