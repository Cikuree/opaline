# Changelog

## Opaline 0.3.0 : jouer via le Launcher Minecraft
- **Second mode de lancement** : sans connexion Opaline, JOUER prépare le pack, crée le profil « Opaline · <pack> » dans le Launcher Minecraft officiel (présélectionné) et l'ouvre ; le compte et le skin y sont gérés. Version dédiée `opaline-<pack>` dans `.minecraft`, fichiers Forge/NeoForge copiés, autres profils conservés (sauvegarde `launcher_profiles.json.opaline.bak`), refus si le Launcher est déjà ouvert.
- **Connexion Microsoft facultative** : en attendant la validation de l'app par Mojang, message explicite et bascule en mode Launcher Minecraft ; dès la validation, la connexion redonne le lancement direct, sans nouvelle version.
- **Pseudo Minecraft** : skin et tête affichés depuis les données publiques de Mojang, sans connexion ; la garde-robe se débloque avec la connexion.
- Page de connexion du navigateur sans pastille de statut.

## Opaline 0.2.1 : tous les loaders, JOUER au menu
- **Loaders** : Fabric, Quilt, NeoForge et Forge, installés depuis leurs sources officielles (installeur et processors exécutés par Opaline, hashes vérifiés, hôtes en liste blanche, redirections revérifiées, installation idempotente et verrouillée).
- **JOUER** : lance le jeu au menu principal de l'instance choisie ; le serveur du pack est ajouté à la liste Multijoueur. Serveur Tailscale (nom MagicDNS court), public ou absent.
- **Packs** : publiés sur la release permanente `packs` à chaque push, sans tag d'Opaline ; catalogue à adresse fixe.
- Opaline Atelier, l'outil de publication de packs, vit désormais dans son propre dépôt.

## Opaline 0.2.0 : launcher Tauri multi-pack
- **Application** :
  - Opaline remplace l'ancien launcher egui : application Tauri 2 sans bordure Windows, avec l'interface de la maquette validée ;
  - polices Bodoni Moda (salutation) et Bricolage Grotesque, embarquées dans l'application ;
  - le cœur Rust devient la bibliothèque `opaline_core`.
- **Packs** :
  - un dossier par pack (`packs/<id>/` + `opaline.toml`), catalogue `packs.json`, une instance isolée par pack, Java et assets partagés entre instances ;
  - changelog des mods (ajoutés, retirés, mis à jour) lors d'une mise à jour ;
  - guide pour ajouter un pack dans `docs/PACKS.md`.
- **Fonctionnalités** :
  - garde-robe de skins et capes via l'API officielle ;
  - têtes des amis et joueurs vus en ligne ;
  - textures Minecraft extraites du `client.jar` du joueur, jamais redistribuées ;
  - historique des morts via le nouvel endpoint `GET /history` du mod.
- **Sécurité** :
  - serveur revérifié (ping et MOTD) juste avant chaque lancement ;
  - jeu et installation exclusifs pour un même pack ;
  - ouverture de liens limitée à des hôtes exacts, CSP stricte ;
  - données du serveur échappées dans l'interface ;
  - jetons et adresse du serveur masqués dans les erreurs et les logs ;
  - variables de développement désactivées en version publiée.
- **Release** :
  - installeur Windows (NSIS) et AppImage Linux, un `pack-<id>.zip` par pack, `packs.json`, `launcher.json` ;
  - avertissement en CI si l'ID client Microsoft manque.

## Launcher en ligne, shaders et distribution
- Pack : Sodium 0.9.2, Iris 1.11.7 et Complementary Reimagined r5.9.3 côté client. Le shaderpack est référencé par lien Modrinth et crédité dans `CREDITS.md`.
- Release unique (tag `v*`), avec ces fichiers :
  - le jar du mod ;
  - les launchers Windows et Linux ;
  - le zip Prism ;
  - `pack.zip`, qui contient le jar référencé avec son sha512 ;
  - `launcher.json` (version, URLs, `pack_hash`).
- Launcher, installation et mises à jour :
  - pack téléchargé depuis la dernière release, avec vérification des hashes de bout en bout et taille comme décompression bornées ;
  - bandeau de mise à jour du launcher ;
  - détection d'une nouvelle version du pack au moment de Jouer.
- Launcher, connexion Microsoft obligatoire :
  - connexion par code d'appareil, vérification de la possession du jeu ;
  - jeton de renouvellement dans le trousseau système, option « Ne pas mémoriser » ;
  - jetons absents des logs et des erreurs ;
  - l'ID client Azure est injecté à la compilation (variable `MS_CLIENT_ID`).
- Launcher, réglages et serveur :
  - modes de performance Faible, Équilibré, Qualité et Personnalisé, avec fusion non destructive de `options.txt` et d'`iris.properties`, et RAM plafonnée à 70 % ;
  - statut du serveur par ping Minecraft. L'adresse `home-server.<suffixe MagicDNS>` est calculée à l'exécution et n'est jamais affichée. Jouer n'est autorisé que si le MOTD contient « Classroom Run ».
- HUD : barre des splits centrée en haut, sous les barres de boss.
- Tableau de la salle sur trois lignes, splits en français.
- Maquette de l'interface Opaline dans `design/launcher-mockup/`.

## Correctif : salle de classe vide
- La palette de `classroom.nbt` utilisait l'ancien format (`Name` / `Properties`). Minecraft 26.3 lit `id` / `properties`, et tous les blocs étaient chargés comme de l'air, sans erreur.
- Le générateur écrit désormais le nouveau format et le `.nbt` est régénéré.
- Un `classroom.nbt` externe à l'ancien format est converti au chargement.
- Le serveur journalise le nombre de blocs posés, et une erreur si aucun ne l'est.

## Retrait du replay
- Retrait complet de ServerReplay et de ReplayMod : plus d'enregistrement, de marqueurs, de route `GET /replay`, de téléchargement ni de bouton « Voir le replay ».
- `HistoryPayload` perd `httpPort` et le JSON de détail perd le champ `replay` (`CONTRACT.md` mis à jour). Les anciens journaux qui contiennent `replay` restent lisibles : le champ est ignoré.
- Le journal des morts et l'écran « Historique des morts » restent (runs, morts, contexte, graphe des vies).
- Le pack packwiz perd `replaymod`, `server-replay` et `fabric-language-kotlin`. Le serveur HTTP ne sert plus que `GET /stats`.

## Étape 2 — journal, replay et launcher
- **Contrat** : `HistoryRequestPayload` (client → serveur) et `HistoryPayload` (index des runs et détail des morts en JSON), documentés dans `CONTRACT.md`.
- **Journal des morts** : un `journal.jsonl` par run dans `config/classroomrun/runs/<n>/`.
  - Écrit en append et lisible après un crash.
  - Contient dégâts, soins, morts, splits, début et fin de run.
  - Pour chaque mort : la vie de chaque joueur sur les 10 s précédentes et le joueur qui a porté le coup fatal.
- **Replay** (ServerReplay, optionnel) :
  - un enregistrement par joueur et par run ;
  - marqueurs aux morts et aux splits ;
  - fichiers sauvegardés avant la suppression des mondes ;
  - route `GET /replay` en liste blanche, qui répond 503 pendant une sauvegarde et accepte 2 téléchargements simultanés au plus.
- **Écran client « Historique des morts »** (touche H, `/crhistory`) : runs, puis morts, puis contexte, avec un graphe des vies. Le bouton « Voir le replay » télécharge l'enregistrement, déconnecte le client puis l'ouvre dans ReplayMod au bon moment.
- **Launcher Rust** (`launcher/`, eframe/egui) :
  - installe Java 25 (Adoptium), Minecraft 26.3, Fabric 0.20.1 et les mods du manifeste packwiz ;
  - vérifie tous les hashes et reprend les téléchargements interrompus ;
  - installe dans une instance isolée, avec un marqueur d'installation complète ;
  - lance le jeu hors ligne et le connecte directement au serveur ;
  - affiche les stats de la run.
- `pack/` forcé en LF, pour que les hashes packwiz restent valides sous Windows.

## Étape 1 — MVP (0.1.0)
- Mod Fabric `classroomrun` (Minecraft 26.3, Fabric Loader 0.20.1, Fabric API 0.165.0+26.3, Fantasy 0.8.4+26.3 inclus).
- Contrat réseau serveur → client figé (`CONTRACT.md`) : `RunStatePayload`, `DamagePayload`, `HealPayload`, `DeathPayload`, splits.
- Serveur :
  - vie, absorption et faim partagées ; faim et régénération naturelle portées par un seul joueur ;
  - totem de la victime qui sauve l'équipe ;
  - mort collective sans écran de mort, puis reset après un court délai ;
  - nouveau trio de mondes Fantasy (Overworld, Nether, End) à chaque run, portails Nether et End redirigés par mixins, ancien trio supprimé ;
  - salle de classe collée au spawn (config, puis jar, puis plateforme de secours) ; tableau noir dynamique ;
  - 9 splits (advancements, dimension ou inventaire), chrono cumulé, morts, stats JSON (écriture atomique, reprise sur fichier corrompu) ;
  - `/run start`, `/run reset`, `/run stats` ; endpoint `GET /stats` (port 25580).
- Client :
  - HUD des splits en haut à gauche, avec chrono et morts ;
  - feed des dégâts et soins avec têtes de joueurs au-dessus de la vie ;
  - commande `/crdebug` pour tester le HUD en solo.
- Salle de classe générée par `structure/build_classroom.py` (15×7×12, DataVersion 5023), avec aperçus PNG.
- Modpack packwiz (`pack/`), script d'instance Prism, CI GitHub Actions (build sur chaque push, release privée sur tag `v*`), `docs/INSTALL.md`.

## Étape 0
- Initialisation du dépôt et du `.gitignore`.
