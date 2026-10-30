# Contrat réseau serveur → client

Package `fr.classroomrun.net` (source set `main`, partagé serveur et client). **Seul l'orchestrateur modifie ce package et ce fichier.** Un agent qui a besoin d'un changement le demande dans son rapport.

Enregistrement : `ClassroomRunNetworking.registerPayloads()`, appelé une fois dans `ClassroomRun.onInitialize()` via `PayloadTypeRegistry.clientboundPlay().register(TYPE, CODEC)`.
Envoi serveur : `ServerPlayNetworking.send(ServerPlayer, payload)`.
Réception client : `ClientPlayNetworking.registerGlobalReceiver(TYPE, (payload, context) -> ...)`.

## Splits — `Split`

Ordre d'affichage, l'ordinal sert d'index. `icon` est le chemin d'un item vanilla (`minecraft:<icon>`).

| # | Split | Icône |
|---|---|---|
| 0 | PICKAXE | wooden_pickaxe |
| 1 | IRON_INGOT | iron_ingot |
| 2 | DIAMOND | diamond |
| 3 | NETHER_PORTAL (briquet) | flint_and_steel |
| 4 | NETHER_WART (forteresse) | nether_wart |
| 5 | BLAZE_ROD | blaze_rod |
| 6 | ENDER_EYE | ender_eye |
| 7 | ENTER_END | end_stone |
| 8 | KILL_DRAGON | dragon_head |

## `RunStatePayload` — `classroomrun:run_state`

| Champ | Type | Sens |
|---|---|---|
| totalTimeMs | long | chrono cumulé de toutes les runs, en ms, au moment de l'envoi |
| running | boolean | le chrono tourne ; le client extrapole avec `Util.getMillis()` |
| deaths | int | morts totales de l'équipe |
| runNumber | int | numéro de la run en cours (à partir de 1) |
| currentSplit | int | index du prochain split ; `Split.values().length` quand la run est finie |
| reachedSplits | int | masque de bits (`Split.bit()`) des splits atteints |

Envoyé : à la connexion d'un joueur, à chaque split, mort, démarrage et reset de run, et toutes les 5 s quand le chrono tourne.

## `DamagePayload` — `classroomrun:damage`

| Champ | Type | Sens |
|---|---|---|
| player | UUID | joueur réellement touché (source du dégât partagé) |
| hearts | float | dégâts réellement subis, en cœurs (`damage / 2`), > 0 |

Envoyé à tous les joueurs, **une fois par coup**, pour le joueur touché uniquement. La propagation de la vie aux autres joueurs ne génère pas de payload. Le cumul « même joueur re-touché en moins de 0,5 s » est fait côté client. Le coup fatal n'envoie pas de `DamagePayload` : il est suivi d'un `DeathPayload`.

## `HealPayload` — `classroomrun:heal`

| Champ | Type | Sens |
|---|---|---|
| player | UUID | joueur dont le soin a déclenché le soin partagé |
| hearts | float | vie réellement gagnée, en cœurs, > 0 |

La régénération naturelle n'est pas envoyée (seulement potions, pommes, totem, etc.).

## `DeathPayload` — `classroomrun:death`

| Champ | Type | Sens |
|---|---|---|
| player | UUID | joueur qui a reçu le coup fatal |
| cause | String | id du type de dégât, ex. `minecraft:fall` |
| message | String | message de mort vanilla en texte brut |
| attacker | Optional&lt;String&gt; | nom du joueur ou id du type d'entité attaquante |
| pos | BlockPos | position de la mort |
| dimension | ResourceKey&lt;Level&gt; | dimension (peut être une dimension Fantasy) |

Envoyé une seule fois par run terminée, avant le reset.

## Historique des morts (étape 2)

### `HistoryRequestPayload` — `classroomrun:history_request` (client → serveur)

| Champ | Type | Sens |
|---|---|---|
| runNumber | int | run à détailler, ou `-1` (`INDEX`) pour la liste des runs |

Le serveur limite à une requête par seconde et par joueur, et ignore les autres.

### `HistoryPayload` — `classroomrun:history` (serveur → client)

| Champ | Type | Sens |
|---|---|---|
| runNumber | int | run demandée, ou `-1` pour l'index |
| json | String (≤ 262 144 car.) | JSON d'index ou de détail, schémas ci-dessous |
| httpPort | int | port HTTP du serveur pour télécharger les replays, `0` si désactivé |

Index (`runNumber = -1`), runs triées de la plus récente à la plus ancienne, 200 au plus :

```json
{"runs": [
  {"run": 20, "startedAt": "2026-10-30T18:00:00Z", "durationMs": 3600000,
   "deaths": 1, "bestSplit": "ENDER_EYE", "finished": false}
]}
```

Détail d'une run :

```json
{"run": 20, "startedAt": "2026-10-30T18:00:00Z",
 "deaths": [{
   "at": "2026-10-30T18:59:00Z",
   "player": "uuid", "name": "Alex",
   "cause": "minecraft:fall", "message": "Alex fell from a high place",
   "attacker": null, "fatalBy": "Alex",
   "pos": [12, 64, -30], "dimension": "classroomrun:run_20_overworld",
   "replay": {"path": "<chemin relatif>.mcpr", "offsetMs": 3540000},
   "health": [{"name": "Alex", "samples": [[-10000, 20.0], [-500, 6.0], [0, 0.0]]}]
 }]}
```

- `replay` vaut `null` si la run n'a pas d'enregistrement.
- `replay.path` est un chemin relatif opaque, téléchargeable via `GET http://<hôte du serveur>:<httpPort>/replay?path=<path url-encodé>`.
- Le serveur ne sert que des chemins présents dans un journal, et répond `404` pour tout le reste.
- `replay.offsetMs` est la position dans ce fichier.
- `health[].samples` contient des paires `[ms relatif à la mort (≤ 0), vie]` sur les 10 s précédentes.
- `fatalBy` est le joueur dont le coup a tué l'équipe.
