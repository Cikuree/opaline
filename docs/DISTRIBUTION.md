# Distribution

Tout le projet vit dans un seul dépôt public : https://github.com/Cikuree/classroom-run (code, pack, CI, releases). L'ancien dépôt privé `OddNode-design/classroom-run` est archivé.

## Releases (tag `vX.Y.Z`, publiées par la CI avec `GITHUB_TOKEN`)

| Asset | Contenu |
|---|---|
| `classroomrun-X.Y.Z.jar` | le mod |
| `classroom-launcher-windows.exe` | le launcher Windows |
| `classroom-launcher-linux` | le launcher Linux |
| `classroom-run-prism.zip` | l'instance Prism importable |
| `pack.zip` | le dossier `pack/` publié, avec `mods/classroomrun.pw.toml` généré (URL et sha512 du jar de la release) |
| `launcher.json` | `{"version": "X.Y.Z", "windows": "<url>", "linux": "<url>", "pack": "<url de pack.zip>"}` |

## Fonctionnement du launcher

- Il lit `https://github.com/Cikuree/classroom-run/releases/latest/download/launcher.json` et `.../latest/download/pack.zip`.
- Les hashes sont vérifiés de bout en bout : `pack.toml` → `index.toml` → chaque `.pw.toml` → fichier téléchargé.
- Le shaderpack Complementary Reimagined n'est référencé que par son lien Modrinth. Le zip n'est jamais redistribué (crédits dans `CREDITS.md`).

## Données privées

Ni l'adresse du serveur (Tailscale) ni aucun jeton ne sont publiés. Chaque joueur saisit l'adresse du serveur dans le launcher.
