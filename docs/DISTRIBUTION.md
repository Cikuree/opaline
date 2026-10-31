# Distribution

Tout le projet vit dans un seul dépôt public : https://github.com/Cikuree/opaline (code, packs, CI, releases). L'ancien dépôt privé `OddNode-design/classroom-run` est archivé.

## Releases (tag `vX.Y.Z`, publiées par la CI avec `GITHUB_TOKEN`)

| Asset | Contenu |
|---|---|
| `packs.json` | catalogue des packs (voir ci-dessous) |
| `pack-<id>.zip` | le dossier `packs/<id>/` publié, avec un dossier `pack/` à la racine ; pour `classroom-run`, `mods/classroomrun.pw.toml` généré (URL et sha512 du jar de la release) |
| `classroomrun-X.Y.Z.jar` | le mod |
| `Opaline_X.Y.Z_x64-setup.exe` | installeur Opaline Windows |
| `Opaline_X.Y.Z_amd64.AppImage` | Opaline Linux |
| `launcher.json` | `{"version": "X.Y.Z", "windows": "<url installeur>", "linux": "<url AppImage>"}` |
| `classroom-run-prism.zip` | l'instance Prism importable |

Le format de `packs.json` (`id`, `name`, `description`, `mc`, `loader`, `loaderVersion`, `modsCount`, `thumbnail`, `widgets`, `pack`, `packHash`, `server`) est défini dans `docs/OPALINE_CONTRACT.md` §3. `packHash` est le sha256 de `index.toml`, égal au `[index] hash` de `pack.toml`.

## Fonctionnement d'Opaline

- Il lit `https://github.com/Cikuree/opaline/releases/latest/download/packs.json` et `.../launcher.json` (mise à jour de l'application).
- Les hashes sont vérifiés de bout en bout : `packHash` → `index.toml` → chaque `.pw.toml` → fichier téléchargé.
- Le shaderpack Complementary Reimagined n'est référencé que par son lien Modrinth. Le zip n'est jamais redistribué (crédits dans `CREDITS.md`).
- Ajouter un pack : `docs/PACKS.md`.

## ID client Microsoft

La CI injecte la variable de dépôt `MS_CLIENT_ID` au build (`CLASSROOM_MS_CLIENT_ID`). Si elle est vide, un avertissement est émis et la connexion est désactivée dans l'application publiée.

## Données privées

Ni l'adresse du serveur ni aucun jeton ne sont publiés. Opaline vise `<host>.<suffixe MagicDNS>` calculé à l'exécution via Tailscale, sans jamais l'afficher.
