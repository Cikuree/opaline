# Ajouter un modpack dans Opaline

Un pack est un dossier `packs/<id>/` : un manifeste [packwiz](https://packwiz.infra.link/) plus un fichier `opaline.toml`. Un push sur `main` dans `packs/` suffit : la CI publie le catalogue et le zip sur la release permanente `packs`, sans tag.

## Parcours simple : Opaline Atelier

1. Ouvre Opaline Atelier (application de bureau pour l'hôte).
2. Dépose le dossier du pack (mods, configs, `options.txt`).
3. Remplis la fiche : nom, description, vignette, serveur.
4. Clique sur Publier.
5. Les joueurs voient le pack dans Opaline après quelques minutes.

## Annexe : méthode manuelle (packwiz)

Les sections ci-dessous décrivent la même chose à la main.

Outils : [packwiz](https://packwiz.infra.link/installation/) (installation), [tutoriel](https://packwiz.infra.link/tutorials/creating/getting-started/), [ajout de mods](https://packwiz.infra.link/tutorials/creating/adding-mods/), [mise à jour](https://packwiz.infra.link/tutorials/updating/updating-mods/).

## 1. Créer le pack

`<id>` : minuscules, chiffres et tirets (`mon-pack`).

```
mkdir packs/<id> && cd packs/<id>
packwiz init
```

Réponses : nom du pack, MC = version voulue, loader = **fabric** (seul loader pris en charge ; les autres s'affichent « non pris en charge »). `packwiz init` écrit `pack.toml` et `index.toml`.

## 2. Ajouter des mods

```
packwiz modrinth add <slug>
```

Le `<slug>` est celui de l'URL Modrinth (`modrinth.com/mod/<slug>`). Chaque mod crée un `mods/<slug>.pw.toml` ; le champ `side` (`client`, `server` ou `both`) décide où il est installé : un mod client-seul n'est pas installé sur le serveur. Les shaders vont dans `shaderpacks/` (`packwiz modrinth add <slug>` les détecte).

Après chaque modification, vérifier que l'index est à jour :

```
packwiz refresh
```

(sans packwiz : `python3 scripts/pack-tools.py index --pack packs/<id>`).

## 3. Écrire `opaline.toml`

Dans `packs/<id>/opaline.toml` :

```toml
name = "Mon pack"
description = "Une phrase."
thumbnail = "ender_eye"       # texture item/ ou block/ du client.jar, [a-z0-9_]+
widgets = ["server"]          # parmi : classroomRun, server, news
[server]
host = "monserveur"           # nom MagicDNS court de la machine, [a-z0-9-]+
port = 25565
motd = "Mon pack"             # le MOTD du serveur doit contenir ce texte
statsPort = 0                 # 0 : pas d'endpoint HTTP (le widget classroomRun n'est utile que pour Classroom Run)
```

Valider : `python3 scripts/pack-tools.py catalog --base-url https://github.com/Cikuree/opaline/releases/download/packs/ --out /tmp/packs.json`. Une erreur nomme le fichier et le champ fautif. L'adresse du serveur n'est jamais affichée aux joueurs.

## 4. Serveur du pack (machine Linux + Tailscale)

1. Installer [Tailscale](https://tailscale.com/kb/1031/install-linux) sur la machine, `sudo tailscale up`, activer MagicDNS dans la console d'administration. Le nom court de la machine (`tailscale status`) est le `host` de `opaline.toml` ; inviter les joueurs sur le tailnet.
2. Installer Java (version requise par la version de MC) et le serveur Fabric correspondant à `[versions]` de `pack.toml`.
3. Mettre dans `mods/` du serveur les mods `side = "server"` ou `"both"` du pack.
4. `server.properties` : `server-port` = `port` de `opaline.toml`, `motd` contenant le texte `motd` d'`opaline.toml`.
5. Lancer un serveur par pack, chacun sur son port, et ouvrir ces ports seulement sur l'interface Tailscale (`sudo ufw allow in on tailscale0 to any port <port>`).

## 5. Tester localement

Sans publier : lancer Opaline en développement avec un `packs.json` local ou construire l'instance Prism (`python3 scripts/build-prism-instance.py --pack packs/<id> --jar <jar>`; le jar sert uniquement à Classroom Run). Vérifier : le pack apparaît, Installer fonctionne, le widget serveur passe au vert quand le serveur tourne.

## 6. Publier

1. Fichiers hors Modrinth (configs, `options.txt`, mods introuvables) : les mettre dans `packs/<id>/` (ex. `config/…`) ; ils sont indexés par hash et inclus dans le zip.
2. Commit des fichiers de `packs/<id>/` et push sur `main`. Aucun tag n'est nécessaire.
3. `packs.yml` publie `pack-<id>.zip` et regénère `packs.json` (nombre de mods, `packHash`) sur la release `packs`.

`packHash` ancre `index.toml`, donc tous les fichiers du pack (chaque métafichier est vérifié par son hash). Les versions MC et loader viennent du `pack.toml` publié dans le même zip, lui-même vérifié par ce hash. `catalog` refuse un pack dont `index.toml` ou un métafichier ne correspond pas aux hashes ; la CI réindexe tous les packs avant le catalogue.

## Mettre à jour ou retirer un mod

- Mise à jour : `packwiz update <slug>` (ou `packwiz update --all`), puis `packwiz refresh`.
- Retrait : `packwiz remove <slug>`, puis `packwiz refresh`.
- Commit et push sur `main`. Au lancement, Opaline détecte la nouvelle version (`packHash` différent) et propose la mise à jour ; son journal liste ajouts, retraits et mises à jour.

## Supprimer un pack

`git rm -r packs/<id>`, commit et push sur `main`. Il disparaît du catalogue. Les installations déjà faites restent sur le disque des joueurs ; ils les suppriment depuis le dossier de données d'Opaline. Arrêter aussi le serveur du pack.

## Crédits

Ajouter chaque mod, shader ou ressource redistribué avec son auteur et sa licence dans `CREDITS.md`.
