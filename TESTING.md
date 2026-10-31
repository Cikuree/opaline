# Checklist de test en jeu

Coche chaque ligne et note ce qui ne va pas (capture d'écran bienvenue). Les numéros servent à me répondre (« 2.3 KO : … »).

## 0. Mise en place

Tester en multijoueur sur ta machine : un serveur et un ou plusieurs clients.

1. Compile le jar :

   ```bash
   ./gradlew build
   ```

   Il est produit dans `build/libs/classroomrun-0.1.0.jar` (n'utilise pas le `-sources`).
2. Lance le serveur de dev. Il démarre dans `run/`, avec Fabric API et Fantasy fournis automatiquement :

   ```bash
   ./gradlew runServer
   ```

3. Lance le client, au choix :
   - `./gradlew runClient`, qui ouvre un client de dev au pseudo aléatoire ;
   - une instance Prism 26.3 + Fabric 0.20.1 + Fabric API, avec le jar copié dans `mods/`.
4. Connecte le client au serveur : `localhost`.
5. Mets-toi opérateur depuis la console serveur : `op <pseudo>`.
6. Pour tester à plusieurs joueurs : deux clients, ou des amis connectés sur ton réseau.

## 1. HUD seul (solo, sans serveur)

Dans un monde solo avec le mod :

- [ ] 1.1 `/crdebug state` : la barre des splits apparaît **en haut à gauche**, avec 9 icônes.
  - Les 4 premières sont en couleur.
  - La 5ᵉ (verrue du Nether) est encadrée en rose avec une petite flèche.
  - Les 4 dernières sont grisées.
  - Le chrono cyan part de 2:14:00 et avance.
  - Le crâne affiche 7.
- [ ] 1.2 `/crdebug clear` : la barre disparaît.
- [ ] 1.3 `/crdebug damage 1` : ta tête avec « -1 » rouge au-dessus des cœurs, qui monte un peu et s'efface en environ 1,5 s. `/crdebug damage 0.5` affiche « -0.5 ».
- [ ] 1.4 Deux `/crdebug damage 1` en moins de 0,5 s : une seule entrée « -2 ». Plus lentement : deux têtes côte à côte.
- [ ] 1.5 `/crdebug damage 1 Truc` : tête par défaut, celle d'un joueur absent de la liste Tab.
- [ ] 1.6 `/crdebug heal 1` : « +1 » vert.
- [ ] 1.7 `/crdebug burst` : 3 têtes côte à côte, « -2 », « -2 », « -0.5 ».
- [ ] 1.8 Le feed ne chevauche ni les cœurs, ni l'armure, ni la barre de faim. Teste aussi avec de l'absorption (pomme dorée) et de l'armure.
- [ ] 1.9 La barre des splits ne gêne ni le chat, ni la liste Tab, ni l'écran F3.

## 2. Salle de classe et reset (serveur)

- [ ] 2.1 À la connexion, tu apparais dans la salle de classe, au centre, face au tableau.
- [ ] 2.2 Le tableau affiche « Run #N — M morts — meilleur split : … », lisible depuis les pupitres.
- [ ] 2.3 La salle est complète :
  - parquet, fenêtres, porte double qui s'ouvre normalement vers l'extérieur ;
  - 6 pupitres avec chaises tournées vers le tableau ;
  - bureau du prof, horloge, étagères, 6 coffres.
- [ ] 2.4 Aucune zone sombre : pas de monstre qui apparaît dans la salle la nuit.
- [ ] 2.5 La salle est posée sur le terrain, ni enterrée ni flottante, et la sortie donne sur l'extérieur.
- [ ] 2.6 `/run reset` : nouveau monde (paysage différent), tout le monde est téléporté dans une nouvelle salle. Inventaire, XP, effets et advancements sont vidés.
- [ ] 2.7 `/run stats` affiche le numéro de run, le chrono, les morts et le prochain split.
- [ ] 2.8 Navigateur sur `http://localhost:25580/stats` : du JSON avec les stats.

## 3. Démarrage et chrono

- [ ] 3.1 Avant `/run start` : le chrono est à l'arrêt et tu ne prends aucun dégât (chute, mob).
- [ ] 3.2 `/run start` : le chrono démarre (HUD) et les dégâts redeviennent actifs.
- [ ] 3.3 Le chrono continue entre deux runs (cumulé) et survit à un redémarrage du serveur. Au redémarrage, une nouvelle run commence.

## 4. Vie partagée (2 joueurs minimum)

- [ ] 4.1 A prend un coup : la vie de B baisse d'autant. Tout le monde voit la tête de A avec « -x ».
- [ ] 4.2 A et B touchés au même moment, par exemple par la même explosion : la perte n'est pas doublée.
- [ ] 4.3 Faim : elle descend à la même vitesse qu'en solo, pas plus vite à plusieurs. La saturation est partagée.
- [ ] 4.4 Régénération naturelle : elle remonte à la même vitesse qu'en solo, sans « +1 » affiché.
- [ ] 4.5 Potion de soin instantané, ou pomme dorée : « +x » vert pour celui qui la prend, et toute l'équipe est soignée.
- [ ] 4.6 Absorption (pomme dorée) partagée par tous.
- [ ] 4.7 Un joueur passe de créatif à survie : l'équipe n'est **pas** soignée, et il prend la vie commune.
- [ ] 4.8 Totem tenu par la victime d'un coup fatal :
  - le totem est consommé ;
  - il n'y a pas de reset ;
  - toute l'équipe passe à 1 cœur.
- [ ] 4.9 Totem tenu seulement par un coéquipier de la victime : mort collective.

## 5. Mort collective

- [ ] 5.1 Un joueur meurt :
  - pas d'écran de mort, pas de drop ;
  - environ 3 s de pause ;
  - toute l'équipe est téléportée dans une nouvelle salle (Run #N+1) ;
  - le compteur de morts augmente de 1, au HUD et sur le tableau.
- [ ] 5.2 Mort pendant que des diamants sont posés dans la grille de craft : rien n'est conservé dans la nouvelle run.
- [ ] 5.3 Un joueur déconnecté pendant le reset, qui se reconnecte ensuite : il arrive dans la nouvelle salle, l'inventaire vide.

## 6. Splits et dimensions

- [ ] 6.1 Les splits s'allument dans l'ordre :
  - pioche (miner de la pierre) ;
  - lingot de fer (en cuire un) ;
  - diamant (en miner un).
- [ ] 6.2 **Allumer un portail du Nether au briquet** : il s'allume, et le split briquet valide à l'entrée dans le Nether.
- [ ] 6.3 Le portail mène au Nether **de la run**, et le retour ramène dans l'overworld de la run, pas dans le monde de départ.
- [ ] 6.4 Verrue du Nether (forteresse), bâton de blaze, œil de l'Ender (le lancer) : splits validés.
- [ ] 6.5 Portail de l'End : on arrive dans l'End de la run, avec le dragon et sa barre de vie. Le panneau des splits ne cache pas la barre du dragon.
- [ ] 6.6 Tuer le dragon : le chrono s'arrête, même si un split a été sauté (par exemple pas de diamant miné).
- [ ] 6.7 Sauter dans le portail de sortie : retour dans la salle de classe.
- [ ] 6.8 `/run reset` pendant le combat contre le dragon : la barre du dragon disparaît bien de l'écran.

## 7. Journal et historique des morts (étape 2)

- [ ] 7.1 Après `/run start` puis quelques coups reçus et une mort, le fichier `run/config/classroomrun/runs/<n>/journal.jsonl` existe et contient une ligne par événement.
- [ ] 7.2 La touche **H** (ou `/crhistory`) ouvre l'écran « Historique des morts » avec la liste des runs, de la plus récente à la plus ancienne.
- [ ] 7.3 Clic sur une run : ses morts s'affichent. Clic sur une mort : cause, message, attaquant, position, dimension, « coup fatal par » et graphe des vies sur les 10 s précédentes.
- [ ] 7.4 Arrête le serveur sauvagement (fermer la fenêtre ou tuer le processus) en pleine run, puis relance-le : l'historique de cette run est toujours lisible.
- [ ] 7.5 La touche H se reconfigure dans Options, Commandes, catégorie « Classroom Run ».

## 8. Opaline (launcher)

Mise en place, une fois :

1. Installe tauri-cli :

   ```bash
   cargo install tauri-cli --version "^2" --locked
   ```

2. Compile Opaline depuis `D:\mod_hardcore\opaline\src-tauri` :

   ```bash
   cargo tauri build --debug --no-bundle
   ```

3. Lance l'exécutable produit dans `opaline/src-tauri/target/debug/`.

Tant qu'aucune release n'existe, le catalogue des packs se pointe en dev sur un fichier local. Il est généré par :

```bash
python scripts/pack-tools.py catalog --tag v0.2.0 --repo Cikuree/opaline --out packs.json
```

Puis lance Opaline avec `OPALINE_PACKS_URL` pointant sur ce fichier (variable lue seulement en build debug).

Sans app Azure validée, la connexion Microsoft reste désactivée. Pour tester le reste en dev, définis `CLASSROOM_DEV_OFFLINE=1` (build debug uniquement).

- [ ] 8.1 La fenêtre Opaline s'ouvre sans bordure Windows. La barre du haut déplace la fenêtre. Réduire, Agrandir et Fermer fonctionnent, et le double-clic sur la barre agrandit.
- [ ] 8.2 L'accueil affiche « Bonsoir, <pseudo> » en Bodoni, les cartes de packs, et la carte Serveur avec son statut et la latence. Aucune adresse n'est visible nulle part.
- [ ] 8.3 Tailscale arrêté : message « Tailscale n'est pas connecté », JOUER désactivé. Tailscale connecté et serveur lancé avec `motd=Classroom Run` : « En ligne », JOUER actif.
- [ ] 8.4 Installer Classroom Run :
  - la barre avance sans jamais reculer (Java, Minecraft, assets, loader, mods, textures) ;
  - ensuite les icônes des splits sont les vraies textures Minecraft ;
  - ton dossier `.minecraft` n'est pas touché, tout est dans `%APPDATA%\Opaline`.
- [ ] 8.5 JOUER lance le jeu directement sur le serveur. Pendant la partie, Installer, Mettre à jour et Réparer sont grisés.
- [ ] 8.6 Le jeu est fermé : Opaline revient à l'état normal. Si le jeu plante au démarrage, la fin du log s'affiche, sans jeton ni adresse.
- [ ] 8.7 Serveur avec un autre MOTD : « Serveur inattendu », JOUER refusé.
- [ ] 8.8 Réglages :
  - les modes Faible, Équilibré et Qualité changent la mémoire, qui reste bornée par ta RAM, et s'appliquent au lancement suivant (shaders, distance de rendu) ;
  - Personnalisé ne touche à rien.
- [ ] 8.9 Fichiers & mods : chaque bouton ouvre le bon dossier de l'instance. La liste des mods est marquée « vérifié ». Réparer revérifie tout.
- [ ] 8.10 Historique : la liste des runs et le détail d'une mort (cause, position, graphe des vies) viennent du serveur. Endpoint `/history`, port 25580.
- [ ] 8.11 Amis : les joueurs connectés au serveur apparaissent avec leur tête de skin et le badge « En jeu ». La colonne s'élargit sans décalage vertical.
- [ ] 8.12 Compte, une fois l'app Azure validée :
  - connexion par code : le code s'affiche, « Ouvrir la page » ouvre microsoft.com/link ;
  - ton skin apparaît dans la garde-robe ;
  - importer un PNG 64×64 puis l'appliquer change ton skin en jeu ;
  - déconnexion.
- [ ] 8.13 Coupe Internet puis relance Opaline : il démarre quand même (catalogue en cache) et joue avec le pack déjà installé.

## Pour me répondre

Indique les numéros KO, avec ce que tu as vu. Les logs serveur sont dans `run/logs/latest.log`, et les logs client dans `.minecraft/logs/latest.log` de l'instance.
