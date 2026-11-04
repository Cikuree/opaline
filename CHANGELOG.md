# Changelog

## Opaline 0.6.11
- Garde-robe : importer un skin en le choisissant fonctionne (l'aperçu de l'image était bloqué par la sécurité de l'application).

## Opaline 0.6.10
- Garde-robe : glisser un skin PNG sur la fenêtre fonctionne (l'application ne recevait pas le fichier).

## Opaline 0.6.9 et mod 0.4.9
- Mod : nouvelle salle d'attente (murs verts à poutres, cheminée, salle à manger, verdure) avec un panneau de stats face au spawn : run, chrono, morts, meilleur split, top 3 des morts par joueur.
- Mod : le clip de mort rejoue aussi les blocs (cratère d'explosion, blocs posés ou cassés) ; sous-titre de mort raccourci.
- Mod : le chrono ne semble plus avancer quand personne n'est connecté.
- Accueil : « Mon temps de jeu » (temps personnel) sur tous les packs ; chrono total sans secondes.
- Shaders : recherche sans filtre de version et possible jeu ouvert ; Mes mods : « Ajouter » revient après « Retirer ».

## Opaline 0.6.8
- Accueil : le temps total s'affiche en heures et minutes (« 12 h 34 »).
- Accueil : la ligne de temps de jeu ne colle plus aux boutons en fenêtre réduite.

## Opaline 0.6.7 et mod 0.4.5
- Mod : clip de mort. À la mort de l'équipe, chacun revoit la dernière minute autour du mort (caméra en orbite, timeline, vitesse x1/x0.5) ; la nouvelle run se prépare quand tout le monde a cliqué « Prêt » (au plus 3 min). Touche R pour rouvrir le clip.
- Mod : le jour se lève à chaque nouvelle run.
- Historique : morts par joueur ; le détail montre la mort qui a fini la run et suit le défilement.
- JOUER ne rejoint plus le serveur et Opaline ne se réduit plus pendant le jeu, sauf si on l'active dans les Réglages.

## Opaline 0.6.6 et mod 0.4.4
- Compte : carte « Mon temps de jeu » (une forme animée par modpack autour du total) ; clic sur sa photo de profil pour ouvrir Compte.
- Connexion Microsoft validée par Mojang : la garde-robe invite à se connecter au lieu d'annoncer l'attente.
- Mod : dégâts et soins affichés à la demi-coeur près (plus de « 2.3 » ni « 0.3 »).
- Mod : la salle de classe se pose au ras du sol, sur la terre ferme (jamais dans l'eau).
- Mod : `/run wipe` remet tout à zéro (chrono, morts, numéro de run, splits) puis prépare une nouvelle run.

## Opaline 0.6.5 : shaders et Fichiers & mods
- Shaders : installer Iris en un clic, chercher un shader sur Modrinth (téléchargé et activé), ou ajouter un .zip.
- Fichiers & mods en onglets : Mods, Shaders & resource packs, Captures, Journal, Instance. Clic droit sur un onglet : ouvrir le dossier ou ajouter un fichier.
- Galerie des captures d'écran (agrandir, naviguer, copier l'image) et lecture du journal du jeu.
- Packs à installer en pointillés ; onglets en pointillés roses pendant le glisser d'un fichier.

## Opaline 0.6.4 : carte plein écran
- La vue Carte (BlueMap) occupe toute la hauteur disponible.
- Pack Classroom Run : carte BlueMap du serveur.

## Opaline 0.6.3 : serveurs Tailscale partagés
- Un serveur Tailscale partagé depuis le réseau d'un ami (« Shared in ») est trouvé par son nom MagicDNS complet : statut du serveur et liste Multijoueur corrects.

## Opaline 0.6.2 : finitions visuelles
- **Mes resource packs et shaders** : glisser-déposer de .zip dans « Fichiers & mods », type reconnu automatiquement, conservés aux mises à jour.
- **Apparence** : cartes, barre de navigation et colonne d'amis plus sombres (#361617) et légèrement translucides sur le fond animé, sans contour ; fond animé plus doux et plus rose, halo de la souris plus clair ; barre de défilement fine et discrète ; cases à cocher aux couleurs d'Opaline ; texte discret plus lisible sur le fond.
- **Réglages réorganisés** : sections Jeu, Lancement, Compte, Notifications et Discord, Apparence ; lignes compactes avec aide « ? » ; copie des réglages entre packs déplacée dans « Fichiers & mods ».
- Les pages défilent jusqu'au bord bas de la fenêtre (plus de fondu).
- Champ de recherche de mods sans double bordure, « Les deux » sur une ligne.

## Opaline 0.6.1 : la grosse mise à jour
(La 0.6.0 n'a jamais été publiée : build Linux corrigé.)
- **Jouer** : rejoindre le serveur du pack en un clic (Quick Play), fermeture du Launcher Minecraft proposée en mode officiel, plein écran fenêtré en option (Cubes Without Borders géré par Opaline).
- **Packs** : accueil en « Mes packs » / « À installer », installation par bouton, clic droit pour ouvrir le dossier, réparer ou désinstaller (mondes et captures sauvegardés), boutons et menu dans « Fichiers & mods », liste des mods courte avec « Tout afficher ».
- **Mondes** : sauvegarde automatique avant installation, mise à jour, réparation et désinstallation, restauration en un clic.
- **Mes mods** : mods perso par pack, recherche Modrinth avec dépendances installées automatiquement, glisser-déposer de .jar.
- **Crash expliqué** en français avec « Vérifier et réparer », « Ouvrir les logs », « Copier le rapport ».
- **Serveur** : notification quand il repasse en ligne, carte BlueMap intégrée.
- **Réglages** : copie des réglages entre packs, choix du shader, réglage automatique selon le PC, réduction près de l'horloge pendant le jeu, statut Discord, fond animé, « Mémoriser la connexion » et « Rester connecté ».
- **Stats** : temps de jeu et dernière session par pack.
- Interface : salutation dans la police de l'application, clic droit limité aux endroits utiles, icône « Vérifier et réparer » corrigée, vérification des mods mémorisée (changement de pack instantané).

## Opaline 0.5.1 : serveurs hébergés
- **Adresses d'hébergeur** (MineStrator…) : l'enregistrement SRV `_minecraft._tcp` est suivi pour un serveur public sur le port par défaut, comme le fait le jeu ; la carte serveur voit le vrai serveur.
- **MOTD** comparé sans tenir compte des majuscules.
- Libellés des chiffres de l'accueil alignés.

## Opaline 0.5.0 : packs importés de CurseForge
- **Archive d'overrides** : un pack peut fournir ses gros dossiers (resource packs, datapacks, configs) dans une archive publiée sur la release `packs`. Opaline la télécharge (sha256 vérifié), l'extrait de façon sûre dans l'instance et retire à la mise à jour les fichiers qu'elle ne contient plus, sans toucher aux mondes ni aux captures.
- **Mod publié avec les packs** : le mod Classroom Run a sa propre version et part avec le pack, sans nouvelle version d'Opaline.

## Classroom Run 0.4.3 : la mort se voit
- **Mort collective visible** : l'équipe passe en spectateur avec le titre « Vous êtes morts » (cause et compte à rebours) avant la nouvelle run, puis revient en survie ; reconnexion pendant ce temps gérée.
- Délai avant la nouvelle run porté à 6 s (`deathResetDelayTicks` = 120 ; un `config.json` existant garde sa valeur).

## Opaline 0.4.2 : finitions
- **Amis** : la colonne montre les joueurs vus sur le serveur du pack (en ligne, ou dernière connexion), sans ajout manuel et sans soi-même ; repliée, elle s'ajuste à son contenu.
- **Fenêtre** : toute la bande du haut attrape la fenêtre, alignée sur les boutons ; version d'Opaline affichée en bas à droite.
- Croix de la recherche couleur corail, items flottants de l'accueil sans saut.
- Pack Classroom Run : AmbientSounds (avec CreativeCore) et Presence Footsteps, côté client.

## Opaline 0.4.1 : mise à jour depuis le launcher
- **Mise à jour en un clic** : bouton « Mettre à jour » dans la bannière ; Opaline télécharge l'installeur de la release officielle, vérifie son empreinte sha256 (publiée dans `launcher.json`), l'installe puis redémarre. Lien manuel conservé en secours.
- Dernière version à installer à la main : les suivantes se mettent à jour depuis Opaline.

## Opaline 0.4.0 : choix du lancement, amis par pseudo
- **Réglage « Lancer le jeu avec »** : Opaline ou Launcher Minecraft, au choix une fois connecté ; sans connexion, le jeu passe toujours par le Launcher Minecraft.
- **Amis** : ajout et retrait par pseudo Minecraft (tête affichée), fusionnés avec les joueurs vus sur le serveur du pack ; « Ajouter » sur un joueur vu.
- **Interface** : croix de la recherche couleur corail, « Aucun ami en ligne » centré, items flottants de l'accueil sans saut au rafraîchissement.

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
