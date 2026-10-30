# API replay : ServerReplay 3.6.0+26.3 (serveur) et ReplayMod 26.3-2.6.28 (client)

Vérification faite sur les jars exacts (sha512 conformes aux `pw.toml`), avec `javap` sur les classes compilées.
Sources textuelles : branche `26.3` de `senseiwells/ServerReplay` et de `CasualChampionships/arcade` (lib `arcade-replay` embarquée dans le jar ServerReplay, version `0.14.0-beta.22+26.3`). Pour ReplayMod : branche `stable` de `ReplayMod/ReplayMod` (tag 2.6.28 non vérifié, donc signatures prises du jar, sémantique de la branche `stable`).

Légende : **[jar]** = `javap` sur le jar ; **[src]** = code source lu ; **NON TROUVÉ** = absent des jars et des sources consultés.

---

## Identifiants de mod

| Mod | Mod id (fabric.mod.json) | Environnement | Source |
|---|---|---|---|
| ServerReplay | `server-replay` | `*` (serveur) | [jar] `fabric.mod.json` |
| ReplayMod | `replaymod` (`ReplayMod.MOD_ID = "replaymod"`) | `client` | [jar] `fabric.mod.json`, `com.replaymod.core.ReplayMod.MOD_ID` |

Détection : `FabricLoader.getInstance().isModLoaded("replaymod")` (côté client uniquement). ReplayMod est `environment: client`, donc un serveur ne le voit jamais.

---

## 1. ServerReplay : enregistrement par programme

Package : `net.casual.arcade.replay.recorder` (la lib `arcade-replay` est embarquée dans le jar de ServerReplay). ServerReplay ne fournit pas sa propre API d'enregistrement, il appelle celle d'arcade.

### Enregistrement par joueur
- `net.casual.arcade.replay.recorder.player.ReplayPlayerRecorders` (objet Kotlin, méthodes `@JvmStatic`) [jar][src]
  - `create(player: ServerPlayer, directory: Path, format: ReplayFormat = ReplayMod, settings: RecorderSettings = DEFAULT): ReplayPlayerRecorder`
  - `create(server: MinecraftServer, profile: GameProfile, connection: Connection, directory: Path, format, settings): ReplayPlayerRecorder`
  - `has(uuid: UUID, format: ReplayFormat): Boolean`, `has(uuid: UUID)`, `has(player: ServerPlayer)`
  - `get(player: ServerPlayer)`, `get(uuid: UUID)` : `Collection<ReplayPlayerRecorder>`
  - `recorders()`, `closing()` : `Collection<ReplayPlayerRecorder>`
  - `stop(uuid: UUID)`
  - Exemple : `ReplayPlayerRecorders.create(player, path, ReplayFormat.ReplayMod, settings).start()`
  - Usage réel : `ReplayCommand.kt` (`startPlayerRecorders`) et `AutomaticRecorders.kt` (`startPlayer`) [src].

### Enregistrement par zone de chunks
- `net.casual.arcade.replay.recorder.chunk.ChunkArea(level: ServerLevel, from: ChunkPos, to: ChunkPos)` [jar][src]
  - `ChunkArea.of(level, x, z, radius)` (utilisé par `ReplayCommand.kt`, `startChunkRecorderAround`) [src]
- `net.casual.arcade.replay.recorder.chunk.ReplayChunkRecorders` (objet Kotlin) [jar][src]
  - `create(area: ChunkArea, directory: Path, format: ReplayFormat, settings: RecorderSettings, position: Vec3?, name: String): ReplayChunkRecorder` (les variantes sans `position`/`name` existent aussi)
  - `has(name: String): Boolean`, `get(name: String): ReplayChunkRecorder?`
  - `containing(level: ResourceKey<Level>, chunk: ChunkPos)`, `intersecting(level, box: BoundingBox)`
  - `recorders()`, `closing()`, `createNameFor(area)`
  - Exemple : `ReplayChunkRecorders.create(ChunkArea(level, from, to), path, ReplayFormat.ReplayMod, settings, null, "zone1").start()`

### Démarrer, arrêter, état (méthodes du recorder, `net.casual.arcade.replay.recorder.ReplayRecorder`) [jar][src]
- `start(mode: StartingMode = StartingMode.Start): Boolean` (`StartingMode` = `Start` | `Restart`)
- `pause(force: Boolean = false): Boolean`, `resume(): Boolean`
- `stop(save: Boolean = true): CompletableFuture<Long>` : asynchrone, la future se complète avec la taille en octets du `.mcpr` final, échoue si déjà arrêté.
- État : `getStopped(): Boolean`, `getPaused(): Boolean`, `isInitialized(): Boolean`, `getStarted$...` (interne, ne pas utiliser)
- Actif ? : `ReplayPlayerRecorders.has(uuid, format)` ou `ReplayChunkRecorders.has(name)`. Ces listes ne contiennent que les enregistrements non fermés (`closing()` = en cours de sauvegarde).

### Une dimension entière / plusieurs joueurs dans un fichier
- **Plusieurs joueurs dans un fichier** : oui, indirectement. Un `ReplayChunkRecorder` enregistre les entités présentes dans la zone (`shouldTrackEntity` filtre par `entity.level().dimension()` et par chunk) [src `ReplayChunkRecorder.kt`]. Un `ReplayPlayerRecorder` = un joueur, un fichier. Non testé en runtime.
- **Dimension entière** : NON TROUVÉ d'option dédiée. Il faut construire un `ChunkArea` aux bornes couvrant la dimension (coordonnées en chunks). Non testé (perf et taille du fichier inconnues).

---

## 2. Marqueurs dans le `.mcpr`

Classe : `net.casual.arcade.replay.recorder.ReplayRecorder` [jar][src]
- `addMarker(name: String?)` : `addMarker(String)` (jar) — utilisé par `ReplayCommand.kt` (`recorder.addMarker(name)`).
- `addMarker(name: String, color: Int)` (jar : `addMarker(String, int)`) : timestamp = temps courant d'enregistrement.
- `addMarker(marker: ReplayMarker)` (jar) avec `net.casual.arcade.replay.util.ReplayMarker(name: String?, position: Vec3?, rotation: Vec2?, timestamp: Duration, color: Int)`.
- Forme complète (Kotlin, `@JvmOverloads`) : `addMarker(name: String?, position: Vec3, rotation: Vec2, timestamp: Duration, color: Int)`. Le `timestamp` vaut `getRecordingLength()` par défaut.
- Écriture effective : `ReplayModWriter.writeMarker` écrit un `com.replaymod.replaystudio.data.Marker` dans les métadonnées du replay [src `ReplayModWriter.kt`].

Exemple : `recorder.addMarker("Début du raid", 0xFF0000)`

---

## 3. Fichiers, chemins, horodatage

### Configuration
- Fichier : `config/server-replay/config.json` (et NON `config/ServerReplay/`). Source : `ReplayConfig.kt`, `root = FabricLoader.getInstance().configDir.resolve("server-replay")`, `file = JsonConfigFile(root/config.json)`. Un ancien dossier `ServerReplay` est migré automatiquement (`migrateOldConfigs`) [src].
- Clés utiles (src `ReplayConfig.kt`, noms exacts `@SerialName`) : `default_encoding` (`ReplayMod` | `Flashback`), `chunk_recording_path`, `player_recording_path`, `player_recording_name` (`{uuid}` / `{username}`), `max_duration`, `restart_after_max_duration`, `max_file_size`, `restart_after_max_file_size`, `system_storage_low_limit`, `recover_unsaved_replays`, `delete_replays_after_duration`, `chunk_recorder_load_radius`, `chunk_recording_strategy` (`always`, `chunk_loaded`, `chunk_contains_player`, `chunk_contains_non_spectator_player`), `automatically_record`, `chunks` (liste `{name, dimension, from_x, from_z, to_x, to_z}`), `player_predicate`, `include_resource_packs`, `record_hotbar`, `record_voice_chat`, `world_name`, `server_name`, `debug`.
- `ServerReplay.config` (objet `me.senseiwells.replay.ServerReplay`, `getConfig()`), `ServerReplay.updateConfig { ... }`.

### Dossiers par défaut
- `<dossier jeu>/recordings/players/` (`player_recording_path`), `<dossier jeu>/recordings/chunks/` (`chunk_recording_path`) [src : `configDir.resolveSibling("recordings")`].
- Joueur : `recordings/players/{uuid}/<date>.mcpr`. Chunks : `recordings/chunks/<name>/<date>.mcpr`.

### Format et nom
- Date : `yyyy-MM-dd--HH-mm-ss` en heure locale du serveur (`DateTimeUtils.getFormattedDate`), suffixe incrémental si collision (`FileUtils.findNextAvailable`). [src]
- Pendant l'enregistrement, le fichier de travail est `recordings/.../<date>` (sans extension). À l'arrêt avec sauvegarde, il devient `<date>.mcpr`. [src `ReplayWriter.kt`, `ReplayModWriter.kt` : `getOutputPath() = path.parent.resolve(path.name + ".mcpr")`]

### Finalisation (asynchrone)
- `recorder.stop(save = true)` renvoie `CompletableFuture<Long>` : le `.mcpr` est complet quand la future se termine. Le travail a lieu sur un executor dédié. [src]
- Événements : `ReplayRecorderSaveEvent` (PRE puis POST) porte `output: Path` ; `ReplayRecorderStopEvent` porte `closeFuture`.

### Chemin du fichier produit
- Avant l'arrêt : `recorder.location` (`public val location: Path`, chemin de travail sans `.mcpr`) ; sa valeur finale est `location.parent.resolve(location.name + ".mcpr")` (c'est exactement `ReplayWriter.getOutputPath()`, public dans l'interface, `ReplayModWriter` override). [src]
- Pendant la sauvegarde : `ReplayRecorderSaveEvent.output` (phase `PHASE_PRE` / `PHASE_POST`). [src]

### Horodatage courant (temps écoulé en ms)
- `recorder.getRecordingLength(): kotlin.time.Duration` [jar `getRecordingLength-UwyO8pc`, src] : durée réelle depuis le début, pauses déduites.
- En ms : `recorder.getRecordingLength().inWholeMilliseconds` (Kotlin). Côté Java, `Duration` est un `long` encodé, préférer une petite fonction Kotlin.
- Ce même timestamp sert aux marqueurs et au `ReplayMod` timeline (zéro = début d'enregistrement).

---

## 4. Dimensions créées à la volée (Fantasy) et suppression

- Une `ChunkArea` reçoit un `ServerLevel` concret à la création. Si la dimension n'est pas chargée, `server.getLevel(key)` renvoie `null` et `AutomaticRecorders.startChunks` journalise `Unable to find dimension ...` puis ignore la zone [src `AutomaticRecorders.kt`]. Donc une dimension Fantasy doit être **chargée** avant de créer l'enregistrement.
- Suppression de dimension pendant l'enregistrement : NON TROUVÉ dans ServerReplay et arcade. Le recorder garde la référence `ChunkArea.level`, aucun listener de déchargement de `ServerLevel` vu. Comportement réel non vérifié (à tester avant de compter dessus).
- Joueur qui change de dimension : `ReplayPlayerRecorder.level` = `player.level()` (dynamique) [src]. Le recorder suit donc le joueur. Les paquets de respawn/changement de monde sont enregistrés par la connexion (aucun handler explicite trouvé, NON TROUVÉ). Non testé.
- Pour une zone de chunks : une entité n'est enregistrée que si elle est dans la même dimension que la zone (`ChunkArea.contains(level, pos)`) [src].

---

## 5. Événements exposés

Système : arcade-event-registry, pas Fabric. Enregistrement comme dans `RecorderNotifier.kt` :

```kotlin
import net.casual.arcade.events.GlobalEventHandler
import net.casual.arcade.events.utils.register
GlobalEventHandler.Server.register<ReplayRecorderStartEvent> { (recorder, mode) -> ... }
```

Événements (package `net.casual.arcade.replay.events`) [jar][src] :
- `ReplayRecorderStartEvent(recorder, mode: StartingMode)`
- `ReplayRecorderStopEvent(recorder, closeFuture: CompletableFuture<Long>)`
- `ReplayRecorderSaveEvent(recorder, output: Path)` : `PHASE_PRE`, `PHASE_POST` (passer `phase = ...` à `register`)
- `ReplayRecorderCloseEvent(recorder)`
- `ReplayRecorderDurationLimitEvent`, `ReplayRecorderFileSizeLimitEvent`, `ReplayRecorderSystemStorageLowEvent`
- `chunk.ReplayChunkRecorderLoadedResumeEvent`, `chunk.ReplayChunkRecorderUnloadedPauseEvent`, `chunk.ReplayChunkRecorderSnapshotEvent`
- `player.ReplayPlayerRecorderSnapshotEvent`

---

## 6. ReplayMod (client) : ouvrir un replay à un instant donné

Package : `com.replaymod.replay` [jar][src]
- `ReplayModReplay` : `public static ReplayModReplay instance` ; `startReplay(java.io.File) throws IOException` ; `startReplay(ReplayFile)` ; `getReplayHandler(): ReplayHandler`.
- `ReplayHandler` : `doJump(int targetTime, boolean retainCameraPosition)` ; `getReplaySender(): ReplaySender` ; `getReplayDuration(): int`.
- `ReplaySender` (interface) : `jumpToTime(int value)` (asynchrone) ; `currentTimeStamp(): int` ; `setAsyncMode(boolean)`.
- Note `doJump` ne fait rien si le sender n'est pas en mode asynchrone (`if (!getReplaySender().isAsyncMode()) return;`) [src branche `stable`].
- Thread : `ReplayMod.instance.runLater(Runnable)` / `runSync(Runnable)` existent [jar]. Le thread requis pour `startReplay` n'est pas vérifié, à utiliser par précaution via `runLater`.

Exemple (client, une ligne d'usage) :
`ReplayModReplay.instance.startReplay(file); ReplayModReplay.instance.getReplayHandler().doJump(ms, false);`

Dossier par défaut des replays client : `./replay_recordings/` (valeur par défaut du setting `recordingPath`, `Setting.RECORDING_PATH`) [jar `com.replaymod.core.Setting`], résolu par `ReplayFoldersService.getReplayFolder()` [jar]. `getRecordingFolder()` résout un sous-dossier `recording` (bytecode, non vérifié visuellement).

---

## 7. Détection de ReplayMod

- `FabricLoader.getInstance().isModLoaded("replaymod")` (côté client, mod id exact `replaymod`). [jar `fabric.mod.json`, `ReplayMod.MOD_ID`]
- Côté serveur : ne pas utiliser, ReplayMod est client-only.

---

## Verdict

- ServerReplay 26.3 : API arcade publique et exploitable par programme (start/stop/pause/marqueurs/événements/chemin). Points non vérifiés : suppression de dimension, suivi multi-joueurs réel, dimension entière.
- ReplayMod 26.3 : `startReplay(File)` + `doJump(int, boolean)` présents dans le jar 2.6.28. Sémantique tirée de la branche `stable`, pas du tag exact.

Sources consultées :
- Jars : `ServerReplay-3.6.0+26.3.jar` (et imbriqué `META-INF/jars/arcade-replay-0.14.0-beta.22+26.3.jar`), `replaymod-26.3-2.6.28.jar`
- https://github.com/senseiwells/ServerReplay (branche 26.3) : `ReplayCommand.kt`, `ReplayConfig.kt`, `AutomaticRecorders.kt`, `RecorderNotifier.kt`, `README.md`
- https://github.com/CasualChampionships/arcade (branche 26.3) : `arcade-replay/.../recorder/ReplayRecorder.kt`, `.../recorder/chunk/*.kt`, `.../recorder/player/*.kt`, `.../io/writer/*`, `.../util/FileUtils.kt`, `arcade-utils/.../DateTimeUtils.kt`
- https://github.com/ReplayMod/ReplayMod (branche stable) : `ReplayModReplay.java`, `ReplayHandler.java`, `ReplaySender.java`
