# Loaders : installation dans Opaline

Recherche du 2026-10-31. « NON VÉRIFIÉ » = à confirmer pendant le développement.

## Quilt

- Liste des versions : `GET https://meta.quiltmc.org/v3/versions/loader/{game}`, champs `loader.{version, maven, hashes}`.
- Profil de lancement : `GET https://meta.quiltmc.org/v3/versions/loader/{game}/{loader}/profile/json`, format proche de celui de Fabric.
- Testé pour 26.3 : loader 0.20.0-beta.9, `mainClass` = `org.quiltmc.loader.impl.launch.knot.KnotClient`.
- Les libraries du profil n'ont que `name` et `url`, sans sha1. Les hashes ne figurent que dans la liste des versions (jar du loader seulement). Vérification par le fichier `.sha1` du Maven : VÉRIFIÉ (maven.quiltmc.org les publie), c'est ce qu'utilise Opaline. La liste `/v3/versions/loader/{game}` n'est pas triée (0.20.0-beta.9 avant 0.31.0-beta.5) : trier côté client.

## NeoForge

- Versions : `https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml`. Le schéma suit la version du jeu, par exemple 26.3.0.100 et 26.1.2.125 ; 1.21.1 correspond à 21.1.x.
- Installeur : `neoforge-<v>-installer.jar`, qui contient :
  - `install_profile.json`, avec processors, data et libraries ;
  - `version.json`, avec `inheritsFrom` = version du jeu et `mainClass` = `net.neoforged.fml.startup.Client` en 26.3 ;
  - `data/client.lzma`, les binpatches.
- En 26.3, l'installeur a 2 processors :
  - `EXTRACT_FILES`, côté serveur ;
  - `installertools:4.0.17:fatjar --task PROCESS_MINECRAFT_JAR --input {MINECRAFT_JAR} --output {PATCHED} --apply-patches {BINPATCH}`.
- Il n'y a pas de `outputs` sha1, donc aucune vérification n'est possible sur le jar patché.
- Les anciennes versions (21.1.x) ont 9 processors : jarsplitter, AutoRenamingTool, binarypatcher, MCP_DATA, DOWNLOAD_MOJMAPS, MERGE_MAPPING…

## Forge

- Versions : `https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json`. Par exemple, `26.3-latest` = 66.1.2 et `26.3-recommended` = 66.1.0.
- Installeur : `https://maven.minecraftforge.net/releases/net/minecraftforge/forge/<mc>-<v>/forge-<mc>-<v>-installer.jar`.
- Processors côté client :
  - `binarypatcher:1.3.4`, avec `--clean {MINECRAFT_JAR} --output {PATCHED} --apply {BINPATCH} --data --unpatched --store` ;
  - `installertools BUNDLER_EXTRACT`, côté serveur.
- `outputs` donne `{PATCHED}` → `{PATCHED_SHA}` : les sorties sont vérifiables par sha1.

## Installeur officiel en mode headless (écarté)

- L'option `--headless --installClient <dir>` existe probablement : NON TESTÉ.
- L'installeur exige un `launcher_profiles.json` dans le dossier cible, il est compilé en Java 8 et il passe par le réseau. Écarté.

## Stratégie retenue pour Opaline

On exécute nous-mêmes l'`install_profile.json`, comme les launchers open source.

1. Télécharger l'installeur, depuis le Maven officiel. Sa vérification passe par le `.sha1` du Maven.
2. En extraire `install_profile.json`, `version.json` et `data/*`.
3. Télécharger les libraries de l'installeur et du profil.
4. Résoudre les variables : `{MINECRAFT_JAR}`, `{SIDE}=client`, `{ROOT}`, `{LIBRARY_DIR}`, `[maven:coords]`, les entrées `data` côté client, et les chemins `/data/...` extraits du jar.
5. Exécuter chaque processor client avec le Java de l'instance (`java -cp <jar>;<classpath> <Main-Class du manifeste> <args>`), dans l'ordre, dans un dossier de travail de l'instance.
6. Vérifier les `outputs` sha1 quand ils sont fournis (Forge).
7. Lancer à partir de `version.json`, fusionné avec la version vanilla (`inheritsFrom`).

Tout se passe dans l'instance isolée et dans `shared/` : rien n'est écrit dans `.minecraft`.

## packwiz `pack.toml`, section `[versions]`

Clés confirmées dans packwiz (`core/pack.go`, `cmd/init.go`) : `fabric`, `quilt`, `forge`, `neoforge`. Packwiz stocke la version Forge sans préfixe `<mc>-` ; Opaline accepte les deux formes. Une seule clé de loader par pack.

## Précisions d'implémentation (Opaline)

- `{ROOT}` vaut `shared/` (qui contient `libraries/`). Les processors sont du code Java officiel exécuté avec le Java de l'instance : rien n'empêche techniquement un processor d'écrire hors de `shared/` ou de l'instance. La protection est la provenance : installeur et libraries ne viennent que des Maven officiels (liste blanche d'hôtes vérifiée aussi à chaque redirection, https, sans identifiants dans l'URL), vérifiés par sha1.
- Les anciens NeoForge (21.1.x) ont des processors qui téléchargent eux-mêmes (par exemple les mappings Mojang) : ces téléchargements sont faits par le code officiel, hors de notre liste blanche.
- Forge demande de ne pas automatiser son installeur. Opaline ne lance pas l'installeur : elle le télécharge depuis le Maven officiel et exécute ses processors comme les launchers open source (Prism, etc.).
- Vérifié sur Windows : Quilt 0.31.0-beta.5, NeoForge 26.3.0.100 et 21.1.265, Forge 66.1.0 (26.3) et 47.4.28 (1.20.1), installés puis lancés jusqu'au rendu.
- Le jar client vanilla est copié sous `versions/<id du loader>/<id du loader>.jar` : `${version_name}.jar` figure dans l'`ignoreList` de Forge et NeoForge.
- Les libraries du loader portant `rules` ou `natives` sont refusées (message clair) plutôt qu'incluses sans évaluation.
- Un verrou `shared/versions/<id>/.lock` (création exclusive, périmé après 30 minutes) protège l'exécution des processors contre une installation concurrente ; l'application sérialise déjà les installations (`install_lock`) et refuse d'installer un pack en cours de jeu.
