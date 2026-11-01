# Opaline (application)

Interface : `ui/` (HTML, CSS, JS sans étape de build). Backend Tauri : `src-tauri/`. Cœur : `../launcher/`.
Contrat entre les deux : `../docs/OPALINE_CONTRACT.md`.

## Statut Discord

Pendant une partie lancée en direct, Opaline affiche « Joue à <Pack> » avec la durée écoulée et l'icône Opaline, sans bouton.
Rien n'est affiché si Discord n'est pas lancé. Réglage « Statut Discord » pour le couper.
Le mode officiel (Launcher Minecraft) n'affiche pas de statut : Opaline ne possède pas le processus du jeu et ne peut pas le suivre.

L'identifiant d'application Discord est une constante de compilation, `DISCORD_APP_ID` (variable d'environnement lue par `option_env!`).
Sans elle, la fonctionnalité est désactivée proprement (aucun fil, aucune connexion). Pour en créer une :

1. Ouvrir <https://discord.com/developers/applications> et cliquer sur « New Application ».
2. Nom : `Opaline` (c'est ce nom que Discord affiche : « Joue à Opaline »).
3. Copier l'« Application ID » (nombre de 17 à 20 chiffres) dans la page « General Information ». Ce n'est pas un secret.
4. « Rich Presence » > « Art Assets » : téléverser le logo (512 x 512 minimum) sous le nom exact `opaline`. Discord met quelques minutes à le diffuser.
5. Compiler avec l'ID : `DISCORD_APP_ID=<id> cargo tauri build` (PowerShell : `$env:DISCORD_APP_ID="<id>"`).
   En CI : variable de dépôt `DISCORD_APP_ID` (voir `../docs/DISTRIBUTION.md`).
