# Classroom Run : vérifications launcher et extras (Minecraft 26.3, Fabric Loader 0.20.1)

Date des vérifications : 2026-10-31. Sources : API Modrinth v2, GitHub (IrisShaders/Iris branche `26.3`, CaffeineMC/Sodium), piston-meta Mojang, sources Loom `minecraft-clientOnly-13d9796e56-26.3-sources.jar` et `minecraft-common-13d9796e56-26.3-sources.jar`, documentation Microsoft Learn et minecraft.wiki.

## A. Modrinth (game_versions 26.3, loader fabric / iris)

### A1. Sodium (slug `sodium`, project id `AANobbMI`)

- Version retenue : `Sodium 0.9.3-beta.1 for Fabric 26.3`, version_number `mc26.3-0.9.3-beta.1-fabric`, id `g6ju16VJ`, publiée 2026-10-15, type beta.
- Fichier : `sodium-fabric-0.9.3-beta.1+mc26.3.jar`, taille 1912496 octets.
- sha512 : `884a1ef0ce0e69d565d41a1359070a8d4f8589a086b565540defdb4741985053077dfd0f2a56898ce60510720a9f95dd8ba220a9a6340b6e0cf5cb12352e2419`
- URL : `https://cdn.modrinth.com/data/AANobbMI/versions/g6ju16VJ/sodium-fabric-0.9.3-beta.1%2Bmc26.3.jar`
- Dépendances requises : aucune.
- Côté : `client_side` required, `server_side` unsupported (client seulement).
- Version stable alternative : `Sodium 0.9.2 for Fabric 26.3` (version_number `mc26.3-0.9.2-fabric`, id `bAZQdGpg`, release, 2026-09-15, fichier `sodium-fabric-0.9.2+mc26.3.jar`, 1908068 octets, sha512 `f3f260f204b8ce5e8777c2c73f537956755ec61f3817b0b78b5e5568e8aa40f14891f146475075109747665c7c88a45ca0ca9a8df3c68d79c58ecdaa8d8c4b93`). C'est la version épinglée par Iris 1.11.7 (voir A2).

### A2. Iris (slug `iris`, project id `YL57xq9U`)

- Version retenue : `Iris 1.11.7 for Fabric 26.3`, version_number `1.11.7+26.3-fabric`, id `vTN4NRGW`, release, 2026-09-30.
- Fichier : `iris-fabric-1.11.7+mc26.3.jar`, 2850938 octets.
- sha512 : `d8d3312f77fc52a48e4a6cd836fda33669bdebcb4de0b8c0d30adc808b287330a533e0baef355cc1ce6336a26851d9775369a802a4666b7b7a6266af20088f42`
- URL : `https://cdn.modrinth.com/data/YL57xq9U/versions/vTN4NRGW/iris-fabric-1.11.7%2Bmc26.3.jar`
- Dépendance requise : Sodium, version épinglée `bAZQdGpg` (Sodium 0.9.2, project `AANobbMI`). Aucune autre dépendance requise.
- Côté : client seulement (`client_side` required, `server_side` unsupported).
- Version précédente encore listée : `1.11.6+26.3-fabric` (id `bAdKrpw8`), même dépendance.

### A3. Complementary Shaders - Reimagined (slug `complementary-reimagined`, project id `HVnmMxH1`, project_type `shader`)

- 26.3 est listé. Version retenue : `r5.9.3` (version_number `r5.9.3`, id `Bqen1mJX`, release, 2026-09-15). Ses `game_versions` incluent `26.1.2`, `26.2`, `26.3`.
- Loaders déclarés : `iris`, `optifine`. Pas de loader `fabric` (un shaderpack ne se charge pas via Fabric, il passe par Iris).
- Fichier : `ComplementaryReimagined_r5.9.3.zip`, 553397 octets.
- sha512 : `45304b1d7862afdb2177b7e6226fc9c6737911d17a4ad46cc9c519ca4deb935f05072cd337cd8b19a15bb8062b57b54eceec7bdee4cd572d94858e2946d37d85` (vérifié par calcul local sur le zip téléchargé, identique à Modrinth).
- URL : `https://cdn.modrinth.com/data/HVnmMxH1/versions/Bqen1mJX/ComplementaryReimagined_r5.9.3.zip`
- Dépendances : aucune déclarée.
- Côté : `client_side` required, `server_side` unsupported.
- Licence (champ Modrinth) : `LicenseRef-Custom`, texte à `https://github.com/ComplementaryDevelopment/ComplementaryReimagined/blob/main/License.txt` (License Agreement 1.7, daté 2026-08-21 dans le zip).
- Redistribution : la licence distingue deux cas.
  - Section 1.2 : le pack peut être ajouté à un modpack si (a) il est crédité visiblement dans la description du modpack s'il est activé par défaut, (b) le créateur du modpack assume les problèmes, (c) le contenu n'est pas modifié, (d) il est ajouté via les systèmes existants de Modrinth ou CurseForge, et il est interdit de le redistribuer par upload direct de fichier.
  - Section 1.3 : redistribution en « Modified Pack » possible sous conditions (crédit avec lien vers le site officiel, licence laissée dans le pack, etc.).
  - Lecture de notre cas (interprétation, pas un avis juridique) : un manifeste packwiz qui pointe vers le téléchargement Modrinth, sans embarquer le zip dans le dépôt, correspond à 1.2(d). Le zip ne doit donc pas être commité ni servi depuis nos propres ressources. Le crédit (1.2(a)) doit figurer dans la description du modpack.

### A4. Format packwiz d'un shaderpack

- Dossier : `shaderpacks/`. Vérifié dans `modrinth/modrinth.go` du dépôt packwiz : `getProjectTypeFolder` renvoie `shaderpacks` pour `project_type = shader`, et `loaderFolders["iris"] = "shaderpacks"` (idem `optifine`).
- Champs du fichier `.pw.toml` (vérifiés dans `core/mod.go`) : `name`, `filename`, `side` (valeurs `client`, `server`, `both`, vide), `[download]` (`hash-format`, `hash`, `url` optionnel, `mode` optionnel), `[update.modrinth]` avec `mod-id` (id du projet) et `version` (id de la version), vérifié dans `modrinth/updater.go`.
- Pour Complementary : `side = "client"`, `filename = "ComplementaryReimagined_r5.9.3.zip"`, `hash-format = "sha512"`, hash ci-dessus, `mod-id = "HVnmMxH1"`, `version = "Bqen1mJX"`.
- Pour Iris et Sodium : même schéma, dossier `mods/`, `side = "client"`.

## B. Configuration

### B5. Iris : `config/iris.properties` (branche `26.3`)

- Chemin : `IrisPlatformHelpers.getInstance().getConfigDir().resolve("iris.properties")` (`common/src/main/java/net/irisshaders/iris/Iris.java`, ligne ~827). Le dossier config est celui de la plateforme (Fabric : `<gameDir>/config`).
- Format : `java.util.Properties` (`common/src/main/java/net/irisshaders/iris/config/IrisConfig.java`).
- Clés lues et écrites :
  - `shaderPack` : nom du shaderpack dans `shaderpacks/`, passé tel quel. Extension `.zip` incluse pour un zip (déduit de `getShaderpacksDirectory().resolve(name + ".txt")`, pas testé en jeu).
  - `enableShaders` : désactivé uniquement si la valeur vaut `false` (défaut activé).
  - `allowUnknownShaders` (défaut `false`), `enableDebugOptions` (défaut `false`), `disableUpdateMessage` (défaut `false`).
  - `maxShadowRenderDistance` (défaut 32), `colorSpace` (défaut `SRGB`).
- Vérifié : ces clés sont dans la branche `26.3`. Le fichier `iris-excluded.json` est un autre fichier du même dossier.

### B6. Complementary : profils

- Profils déclarés dans `shaders/shaders.properties` du zip r5.9.3 sous la forme `profile.<NOM> = CLE=VAL ...`.
- Valeurs réelles : `POTATO`, `VERYLOW`, `LOW`, `MEDIUM`, `HIGH`, `VERYHIGH`, `ULTRA`, `COMPLEMENTARY`. Les valeurs proposées (POTATO/LOW/MEDIUM/HIGH/ULTRA) sont incomplètes.
- Clé `PROFILE` ou fichier de profil dédié : NON TROUVÉ. Iris ne stocke pas le nom du profil. `ShaderPack.java` détermine le profil courant par `ProfileSet.scan` à partir des valeurs d'options.
- Les valeurs d'options sont persistées dans `shaderpacks/<nom du pack>.txt`, au format `Properties` (`Iris.java`, lignes ~277 et ~352-356). Pour fixer un profil sans UI, il faut écrire dans ce fichier les paires `CLE=VAL` du profil choisi (ex. `SHADOW_QUALITY`, `shadowDistance`, `CLOUD_QUALITY`). Le nom exact du `.txt` pour un zip est déduit (`<fichier zip>.txt`), non testé.

### B7. Sodium : `config/sodium-options.json`

- Nom de fichier : `DEFAULT_FILE_NAME = "sodium-options.json"` (`SodiumOptions.java`, branche `26.2/stable`). Le dossier exact est résolu par `getConfigPath()` (non relu en détail, supposé être le dossier config).
- Sérialisation Gson en `LOWER_CASE_WITH_UNDERSCORES` : les clés sont en snake_case.
- Sections et clés (défauts entre parenthèses) :
  - `quality` : `hidden_fluid_culling` (true), `improved_fluid_shaping` (false), `use_closest_point_entity_sort` (false), `pixel_filtering_mode` (`NEAREST`).
  - `performance` : `chunk_builder_threads` (0), `chunk_build_defer_mode` (`ALWAYS`), `animate_only_visible_textures` (true), `use_entity_culling` (true), `use_fog_occlusion` (true), `use_block_face_culling` (true), `use_no_error_gl_context` (true), `quad_splitting_mode` (`SAFE`).
  - `advanced` : `enable_memory_tracing` (false). `debug` : `terrain_sorting_enabled` (true). `notifications` : champs d'état.
- Limite : le dépôt Sodium n'a pas de branche `26.3` (seulement `26.1.2/stable` et `26.2/stable`). Les clés viennent de `26.2/stable` et sont à revérifier contre la version 0.9.3-beta.1. Les réglages de distance ou de graphismes sont des options vanilla, pas dans ce fichier.

### B8. options.txt vanilla 26.3

Source : `net/minecraft/client/Options.java`, méthode `processDumpedOptions(OptionAccess)` (ligne 1518), appelée depuis `processOptions(FieldAccess)` (ligne 1561).

- `renderDistance` : défaut 12, plage 2 à 16 (32 si `largeDistances`).
- `simulationDistance` : défaut 12, plage 5 à 16 (32 si `largeDistances`). Minimum 2 seulement si `DEBUG_ALLOW_LOW_SIM_DISTANCE`.
- `graphicsMode` : NON TROUVÉ dans 26.3. La clé est `graphicsPreset` (`access.process("graphicsPreset", ...)`), défaut `FANCY`, valeurs sérialisées `fast`, `fancy`, `fabulous`, `custom` (`net/minecraft/client/GraphicsPreset.java`).
- `renderClouds` (et non `cloudStatus`) : valeurs `false` (OFF), `fast` (FAST), `true` (FANCY) (`net/minecraft/client/CloudStatus.java`, `getSerializedName`).
- `cloudRange` : clé présente, défaut non relu.
- `particles` : entier stocké via `ParticleStatus.LEGACY_CODEC`. `0` = ALL (défaut), `1` = DECREASED, `2` = MINIMAL (`net/minecraft/server/level/ParticleStatus.java`, module common).
- `maxFps` : clé présente (`access.process("maxFps", this.framerateLimit)`), défaut 120, codec 10 à 260 (260 = illimité).
- Non vérifié : la migration de `graphicsMode` par DataFix. La compatibilité avec un ancien `options.txt` reste à tester.

## C. Authentification Microsoft (flux device code, application publique)

### C9. Endpoints et paramètres

- Device code : `POST https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode`, paramètres `client_id`, `scope=XboxLive.signin offline_access`. `expires_in` par défaut 900 s (15 min) : Microsoft Learn, `v2-oauth2-device-code`.
- Polling token : `POST https://login.microsoftonline.com/consumers/oauth2/v2.0/token`, `grant_type=urn:ietf:params:oauth:grant-type:device_code`, `client_id`, `device_code`. Erreurs attendues : `authorization_pending`, `authorization_declined`, `bad_verification_code`, `expired_token` (Microsoft Learn). L'erreur `slow_down` n'est pas dans la page Learn lue : NON TROUVÉ dans cette source.
- Refresh : grant `refresh_token` sur le même endpoint `token` (Learn renvoie au flux code OAuth pour ce point, non relu en détail).
- Tenant : `consumers` obligatoire avec `XboxLive.signin`. Un tenant AAD ou `common` provoque des erreurs (minecraft.wiki, « Microsoft authentication »).
- Xbox Live : `POST https://user.auth.xboxlive.com/user/authenticate`. Corps : `Properties.AuthMethod = "RPS"`, `Properties.SiteName = "user.auth.xboxlive.com"`, `Properties.RpsTicket = "d=<access_token>"`, `RelyingParty = "http://auth.xboxlive.com"`, `TokenType = "JWT"` (minecraft.wiki).
- XSTS : `POST https://xsts.auth.xboxlive.com/xsts/authorize`. Corps : `Properties.SandboxId = "RETAIL"`, `Properties.UserTokens = ["<xbl_token>"]`, `RelyingParty = "rp://api.minecraftservices.com/"`, `TokenType = "JWT"` (minecraft.wiki).
- Login Minecraft : `POST https://api.minecraftservices.com/authentication/login_with_xbox`, `identityToken = "XBL3.0 x=<userhash>;<xsts_token>"` (minecraft.wiki).
- Entitlements : `GET https://api.minecraftservices.com/entitlements/mcstore`, `Authorization: Bearer <token MC>`. `items` vide si le jeu n'est pas possédé (minecraft.wiki).
- Profil : `GET https://api.minecraftservices.com/minecraft/profile`, même header (minecraft.wiki).
- Codes XSTS (`XErr`, minecraft.wiki) :
  - `2148916227` : compte banni de Xbox.
  - `2148916233` : pas de compte Xbox, à créer (minecraft.net).
  - `2148916235` : pays où Xbox Live n'est pas disponible.
  - `2148916236` et `2148916237` : vérification d'âge adulte requise (Corée du Sud).
  - `2148916238` : compte mineur, à rattacher à une famille. Apparaît seulement avec une app Azure perso, pas avec le client ID du launcher Minecraft.
  - `2148916262` : NON DOCUMENTÉ (cause peu claire).
- Approbation Mojang : oui, requise. Une app Azure neuve doit demander l'accès à l'API Minecraft via le formulaire `https://aka.ms/mce-reviewappid`. Sans cela, `api.minecraftservices.com` répond 403 (minecraft.wiki, citant un article du support Minecraft `help.minecraft.net/hc/en-us/articles/16254801392141p`, lien non vérifié). Une réponse Q&A Microsoft (non officielle) indique que `XboxLive.signin` demanderait aussi une inscription au programme développeur Xbox : NON CONFIRMÉ.
- Sources officielles : Microsoft Learn (device code), minecraft.wiki « Microsoft authentication ». wiki.vg `Microsoft_Authentication_Scheme` : lecture en échec (ECONNREFUSED), donc NON VÉRIFIÉ depuis wiki.vg.

### C10. Arguments de lancement (version 26.3, piston-meta)

- Version JSON : `https://piston-meta.mojang.com/v1/packages/5df1795e16eea19d75b923d66ecca9ce183353c5/26.3.json`. Champ `arguments.game` (la clé `minecraftArguments` n'existe pas en 26.3).
- Arguments présents : `--username ${auth_player_name}`, `--version ${version_name}`, `--gameDir`, `--assetsDir`, `--assetIndex`, `--uuid ${auth_uuid}`, `--accessToken ${auth_access_token}`, `--clientId ${clientid}`, `--xuid ${auth_xuid}`, `--versionType ${version_type}`.
- Source `net/minecraft/client/main/Main.java` : `--uuid` (requis en pratique), `--xuid` (défaut vide), `--clientId` (défaut vide), `--accessToken` (requis), `--versionType` (défaut `release`).
- `--userType` : NON TROUVÉ, ni dans le JSON ni dans `Main.java`. `Main` appelle `parser.allowsUnrecognizedOptions()`, donc un `--userType` passé par le launcher serait ignoré sans erreur.

## Points ouverts

- Sodium 0.9.3-beta.1 : clés `sodium-options.json` à revérifier sur une source 26.3 (absente du dépôt).
- Chemin exact du `.txt` de options Complementary pour un zip : à tester en jeu.
- Migration `graphicsMode` vers `graphicsPreset` (DataFix) : non vérifiée.
- Approbation Azure : délai et conditions réels non documentés publiquement.
