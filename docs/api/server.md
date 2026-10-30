# API serveur : Minecraft 26.3 / Fabric API 0.165.0+26.3 / Fantasy 0.8.4+26.3

Vérifié le 2026-10-30 par fabric-scout. Aucun nom n'est deviné : chaque entrée cite son jar ou sa source.

Sources utilisées :
- **MC** : sources décompilées `D:\mod_hardcore\.gradle\loom-cache\minecraftMaven\net\minecraft\minecraft-common-13d9796e56\26.3\minecraft-common-13d9796e56-26.3-sources.jar` (noms officiels, non obfusqués).
- **FA** : `~/.gradle/caches/modules-2/files-2.1/net.fabricmc.fabric-api/fabric-api/0.165.0+26.3/.../fabric-api-0.165.0+26.3.jar`, puis jars imbriqués (`META-INF/jars/`). Pas de sources pour 26.3, signatures via `javap`.
- **FT** : `xyz.nucleoid/fantasy/0.8.4+26.3/.../fantasy-0.8.4+26.3.jar` (`javap`) + README `github.com/NucleoidMC/fantasy` (branche `master`, la branche `26.3` renvoie 404).
- **MCJ** : `minecraft-common-13d9796e56-26.3.jar` et `~/.gradle/caches/fabric-loom/26.3/minecraft-server.jar`/`minecraft-common.jar`.

## Corrections par rapport à la demande

- Fantasy 0.8.4+26.3 n'a pas `RuntimeWorldConfig` / `RuntimeWorldHandle`. Les classes réelles sont `RuntimeLevelConfig` et `RuntimeLevelHandle`. Vocabulaire « level », pas « world ».
- `getOrOpenPersistentWorld` → `getOrOpenPersistentLevel(Identifier, RuntimeLevelConfig)`.
- `openTemporaryWorld` → `openTemporaryLevel(RuntimeLevelConfig)` ou `openTemporaryLevel(Identifier, RuntimeLevelConfig)`.
- `asWorld()` n'existe pas → `asLevel()`.
- Fabric API 26.3 n'a pas d'événement de soin : `ServerLivingEntityEvents` ne contient que ALLOW_DAMAGE, AFTER_DAMAGE, ALLOW_DEATH, AFTER_DEATH, MOB_CONVERSION.
- Advancements : le type est `AdvancementHolder`, pas `Advancement`. Les IDs se lisent `minecraft:story/mine_stone`, etc.

---

## 1. Événements Fabric

Tous les événements sont des champs `public static final Event<...>`. Usage : `Event.register(lambda)`.

**ServerLivingEntityEvents** (FA, `net.fabricmc.fabric.api.entity.event.v1`)
- `ALLOW_DAMAGE` : `Event<ServerLivingEntityEvents$AllowDamage>`. Méthode : `boolean allowDamage(LivingEntity, DamageSource, float)`. Retourner `false` annule les dégâts.
  Usage : `ServerLivingEntityEvents.ALLOW_DAMAGE.register((entity, source, amount) -> true);`
  Source : `fabric-entity-events-v1-6.0.4`, mixin `net.fabricmc.fabric.mixin.entity.event.LivingEntityMixin` (injection dans `hurtServer`).
- `AFTER_DAMAGE` : `void afterDamage(LivingEntity, DamageSource, float baseDamage, float damageTaken, boolean blocked)`.
  Usage : `ServerLivingEntityEvents.AFTER_DAMAGE.register((e, src, base, taken, blocked) -> {});`
  Source : même jar.
- `ALLOW_DEATH` : `boolean allowDeath(LivingEntity, DamageSource, float)`.
  Usage : `ServerLivingEntityEvents.ALLOW_DEATH.register((e, src, amt) -> true);`
  Source : mixin `LivingEntityMixin`, méthode `beforeEntityKilled`, injectée dans `hurtServer` au point `isDeadOrDying()`. Le point précis est avant ou après `checkTotemDeathProtection` : NON VÉRIFIÉ (il faut lire les `@At` du mixin). Vu dans l'ordre du code : `isDeadOrDying()` (ligne 1248) puis `checkTotemDeathProtection` (ligne 1249). Les deux sont dans `hurtServer`.
- `AFTER_DEATH` : `void afterDeath(LivingEntity, DamageSource)`. Appelé via `notifyDeath` dans `die`.
  Usage : `ServerLivingEntityEvents.AFTER_DEATH.register((e, src) -> {});`
- Événement de soin : **NON TROUVÉ** dans FA 26.3. Utiliser la méthode vanilla `LivingEntity.heal(float)` (voir section 2).
- `MOB_CONVERSION` : existe, hors besoin.

**ServerPlayerEvents** (FA, `net.fabricmc.fabric.api.entity.event.v1`)
- `COPY_FROM` : `void copyFromPlayer(ServerPlayer oldPlayer, ServerPlayer newPlayer, boolean alive)`.
  Usage : `ServerPlayerEvents.COPY_FROM.register((oldP, newP, alive) -> {});`
- `AFTER_RESPAWN` : `void afterRespawn(ServerPlayer oldPlayer, ServerPlayer newPlayer, boolean alive)`.
  Usage : `ServerPlayerEvents.AFTER_RESPAWN.register((oldP, newP, alive) -> {});`
- `JOIN` : `void onJoin(ServerPlayer)`. `LEAVE` : `void onLeave(ServerPlayer)`.
- `ALLOW_DEATH` : `boolean allowDeath(ServerPlayer, DamageSource, float)`.
  Usage : `ServerPlayerEvents.ALLOW_DEATH.register((p, src, amt) -> true);`

**ServerPlayConnectionEvents** (FA, `net.fabricmc.fabric.api.networking.v1`)
- `JOIN` : `void onPlayReady(ServerGamePacketListenerImpl handler, PacketSender sender, MinecraftServer server)`.
  Usage : `ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> {});` Le joueur est `handler.getPlayer()`.
- `DISCONNECT` : `void onPlayDisconnect(ServerGamePacketListenerImpl handler, MinecraftServer server)`.
- `INIT` : existe, non utilisé ici.

**ServerTickEvents** (FA, `net.fabricmc.fabric.api.event.lifecycle.v1`)
- `START_SERVER_TICK` : `void onStartTick(MinecraftServer)`.
- `END_SERVER_TICK` : `void onEndTick(MinecraftServer)`.
- `START_LEVEL_TICK` / `END_LEVEL_TICK` : `void onStartTick(ServerLevel)` / `void onEndTick(ServerLevel)`.
  Usage : `ServerTickEvents.END_SERVER_TICK.register(server -> {});`

**ServerLifecycleEvents** (FA, `net.fabricmc.fabric.api.event.lifecycle.v1`)
- `SERVER_STARTING` : `void onServerStarting(MinecraftServer)`.
- `SERVER_STARTED` : `void onServerStarted(MinecraftServer)`.
- `SERVER_STOPPING` : `void onServerStopping(MinecraftServer)`.
- `SERVER_STOPPED`, `BEFORE_SAVE`, `AFTER_SAVE`, `SYNC_DATA_PACK_CONTENTS`, `START_DATA_PACK_RELOAD`, `END_DATA_PACK_RELOAD` : existent.
  Usage : `ServerLifecycleEvents.SERVER_STARTED.register(server -> {});`

**CommandRegistrationCallback** (FA, `net.fabricmc.fabric.api.command.v2`)
- `EVENT` : `Event<CommandRegistrationCallback>`. Méthode : `void register(CommandDispatcher<CommandSourceStack> dispatcher, CommandBuildContext registryAccess, Commands$CommandSelection environment)`.
  Usage : `CommandRegistrationCallback.EVENT.register((dispatcher, registry, env) -> {});`
  Source : `fabric-command-api-v2-3.1.2`.

**ServerEntityCombatEvents** (FA, `net.fabricmc.fabric.api.entity.event.v1`), bonus
- `AFTER_KILLED_OTHER_ENTITY` : `void afterKilledOtherEntity(ServerLevel, Entity, LivingEntity, DamageSource)`.

## 2. Vie, faim, absorption, DamageSource, mort

Package : `net.minecraft.world.entity.LivingEntity` (MC, `LivingEntity.java`).
- `void heal(float heal)` : ligne 1147. Ajoute si `health > 0`. Usage : `entity.heal(4.0F);`
- `float getHealth()` : ligne 1154.
- `void setHealth(float health)` : ligne 1158. Borne entre `0` et `getMaxHealth()`.
  Usage : `living.setHealth(20.0F);`
- `final float getMaxHealth()` : ligne 1984. Lit `Attributes.MAX_HEALTH`.
- `float getAbsorptionAmount()` : ligne 3371.
- `final void setAbsorptionAmount(float)` : ligne 3375. Borne par `getMaxAbsorption()`.
  Usage : `living.setAbsorptionAmount(4.0F);`
- `boolean isDeadOrDying()` : ligne 1162.
- `boolean hurtServer(ServerLevel, DamageSource, float)` : ligne 1171. Point d'entrée des dégâts côté serveur.
- `void die(DamageSource)` : ligne 1447.
- `CombatTracker getCombatTracker()` : ligne 1972.

Faim : package `net.minecraft.world.food`, classe `FoodData` (MC, `world/food/FoodData.java`).
- `int getFoodLevel()` : ligne 88.
- `void setFoodLevel(int food)` : ligne 108.
- `float getSaturationLevel()` : ligne 104.
- `void setSaturation(float saturation)` : ligne 112.
- `boolean needsFood()` : ligne 96. `void addExhaustion(float)` : ligne 100.
- `Player.getFoodData()` : `public FoodData getFoodData()`, ligne 1560 de `world/entity/player/Player.java`.
  Usage : `player.getFoodData().setFoodLevel(20);`

DamageSource : package `net.minecraft.world.damagesource`, classe `DamageSource` (MC).
- `@Nullable Entity getEntity()` : ligne 63. Source logique (ex. le joueur qui a tiré).
- `@Nullable Entity getDirectEntity()` : ligne 59. Entité qui a porté le coup (projectile, etc.).
- `DamageType type()` : ligne 124. Type défini par la registry, `net.minecraft.world.damagesource.DamageType`.
- `String getMsgId()` : ligne 88. Identifiant du message de mort (`type.msgId()`).
- `@Nullable Vec3 getSourcePosition()` : ligne 104.
  Usage : `source.getEntity()`, `source.type().msgId()`.

Message de mort : `CombatTracker.getDeathMessage()` : `Component getDeathMessage()`, ligne 88 de `world/damagesource/CombatTracker.java`. Usage : `living.getCombatTracker().getDeathMessage()`.

Totem :
- Méthode vanilla : `private boolean checkTotemDeathProtection(DamageSource killingDamage)`, ligne 1384 de `LivingEntity.java`. Appelée à la ligne 1249 dans `hurtServer`, dans le bloc `if (this.isDeadOrDying())`. Si elle retourne `false`, `die(source)` est appelé (ligne ~1252).
- Ordre : `ALLOW_DEATH` (mixin, injection sur `isDeadOrDying`) est appelé avant `checkTotemDeathProtection` si le point d'injection est avant l'appel. Position exacte : NON VÉRIFIÉE (voir section 1).
- Le totem est ignoré pour les sources `DamageTypeTags.BYPASSES_INVULNERABILITY` (ligne 1385).

## 3. Fantasy 0.8.4+26.3 (FT)

Package : `xyz.nucleoid.fantasy`. Classes réelles : `Fantasy`, `RuntimeLevelConfig`, `RuntimeLevelHandle`, `RuntimeLevel`, `RuntimeLevel$Style` (`PERSISTENT`, `TEMPORARY`), `RuntimeLevel$Constructor`, `FantasyDimensionOptions`, `RuntimeClockManager`.

- `static Fantasy Fantasy.get(MinecraftServer)`.
  Usage : `Fantasy fantasy = Fantasy.get(server);`
- `RuntimeLevelHandle getOrOpenPersistentLevel(Identifier id, RuntimeLevelConfig config)`. Le README précise qu'il faut l'appeler de nouveau après un redémarrage.
- `RuntimeLevelHandle openTemporaryLevel(RuntimeLevelConfig config)` et `openTemporaryLevel(Identifier id, RuntimeLevelConfig config)`.
- `RuntimeLevelConfig` : constructeur public `new RuntimeLevelConfig()`. Setters chaînables :
  - `setSeed(long)`
  - `setDimensionType(Holder<DimensionType>)`, `setDimensionType(DimensionType)`, `setDimensionType(ResourceKey<DimensionType>)`
  - `setGenerator(ChunkGenerator)`
  - `setDifficulty(Difficulty)`, `setGameRule(GameRule<T>, T)`
  - `setFlat(boolean)`, `setShouldTickTime(boolean)`, `setGameTime(long)`
  - `setClockTime(ResourceKey<WorldClock>, int)`, `setMirrorOverworldGameRules(boolean)`, `setMirrorOverworldDifficulty(boolean)`, `setMirrorOverworldClocks(boolean)`, `mirrorOverworldGameState()`
  Usage : `new RuntimeLevelConfig().setSeed(42L).setGenerator(gen);`
- `RuntimeLevelHandle` : `ServerLevel asLevel()`, `ResourceKey<Level> getRegistryKey()`, `void delete()`, `void unload()`, `void setTickWhenEmpty(boolean)`.
  Usage : `handle.asLevel()`, `handle.delete()`.
- `asWorld()` : NON TROUVÉ (c'est `asLevel()`).
- `openTemporaryWorld` / `getOrOpenPersistentWorld` : NON TROUVÉ (noms `*Level`).
- Trio Overworld + Nether + End lié : **NON TROUVÉ**. Aucune référence au Nether ou à l'End dans le jar (`grep` négatif). Le README ne mentionne ni le Nether, ni l'End, ni les portails, ni de monde lié. Chaque appel crée un seul `RuntimeLevel`. Pour les portails dans un monde runtime, la cible vanilla n'est pas garantie : à tester en jeu.

Source README : `https://github.com/NucleoidMC/fantasy` (branche `master`).

## 4. Structures

- Package : `net.minecraft.world.level.levelgen.structure.templatesystem`.
- `StructureTemplateManager` : constructeur `StructureTemplateManager(ResourceManager, LevelStorageAccess storage, DataFixer fixerUpper, HolderGetter<Block> blockLookup)`, ligne 54.
  - `Optional<StructureTemplate> get(Identifier id)` : ligne 89. Lit depuis les ressources (datapack/`data/<ns>/structure/`).
  - `StructureTemplate getOrCreate(Identifier id)` : ligne 78.
  - `Stream<Identifier> listTemplates()` : ligne 93.
  - `boolean save(Identifier id)` : ligne 116. Statique : `save(Path, StructureTemplate, boolean asText)`, ligne 140.
- Lecture d'un `.nbt` depuis un fichier ou un flux (pas de méthode `readStructure` dans 26.3) :
  - `NbtIo.readCompressed(Path file, NbtAccounter accounter)` : `CompoundTag`, `throws IOException`, ligne 31 de `nbt/NbtIo.java`.
  - `NbtIo.readCompressed(InputStream in, NbtAccounter accounter)` : `CompoundTag`, ligne 48.
  - `NbtAccounter.unlimitedHeap()` : `static NbtAccounter`, ligne 31 de `nbt/NbtAccounter.java`.
  - `StructureTemplate.load(HolderGetter<Block> blockLookup, CompoundTag tag)` : `void`, ligne 689 de `StructureTemplate.java`.
  - Usage : `StructureTemplate t = new StructureTemplate(); t.load(server.registryAccess().lookupOrThrow(Registries.BLOCK), NbtIo.readCompressed(path, NbtAccounter.unlimitedHeap()));`
  - `readStructure` : NON TROUVÉ.
- `StructureTemplate.placeInWorld` (ligne 241) :
  `boolean placeInWorld(ServerLevelAccessor level, BlockPos position, BlockPos referencePos, StructurePlaceSettings settings, RandomSource random, @Block.UpdateFlags int updateMode)`.
  Usage : `template.placeInWorld(level, pos, pos, settings, level.getRandom(), Block.UPDATE_CLIENTS);`
- `StructurePlaceSettings` : setters chaînables `setMirror`, `setRotation`, `setRotationPivot`, `setIgnoreEntities(boolean)`, `setBoundingBox`, `setRandom`, `setLiquidSettings`, `setKnownShape`, `setFinalizeEntities`. Constructeur sans argument.

## 5. Entité text_display

- Classe : `net.minecraft.world.entity.Display.TextDisplay`, `public static class TextDisplay extends Display`, `Display.java` ligne 763.
- Setter : `public final void setText(Component text)`, ligne 823. Public, pas privé.
- Getter : `public final Component getText()`, ligne 816.
  Usage : `textDisplay.setText(Component.literal("Salut"));`
- Création : `new Display.TextDisplay(EntityType.TEXT_DISPLAY, level)`. Constructeur non vérifié ligne par ligne ; passer par `EntityType.TEXT_DISPLAY.create(level, EntitySpawnReason.COMMAND)` si besoin (signature `EntitySpawnReason` non vérifiée, à contrôler).
- Tags : `Set<String> Entity.entityTags()` (ligne 405). `boolean Entity.addTag(String)` (ligne 409). `boolean Entity.removeTag(String)` (ligne 413). `getTags()` : NON TROUVÉ.
  Usage : `entity.addTag("pont_hud"); entity.entityTags().contains("pont_hud");`
- `ServerLevel.getEntities` :
  - `<T extends Entity> List<? extends T> getEntities(EntityTypeTest<Entity, T> type, Predicate<? super T> selector)` : ligne 908.
  - `<T extends Entity> void getEntities(EntityTypeTest<Entity, T> type, Predicate<? super T> selector, List<? super T> result)` : ligne 914.
  - `Iterable<Entity> getAllEntities()` : ligne 1780.
  Usage : `level.getEntities(EntityType.TEXT_DISPLAY, e -> e.entityTags().contains("pont_hud"));`

## 6. Advancements et inventaire

- Événement Fabric « advancement obtenu » : **NON TROUVÉ**. FA 26.3 ne fournit que `AdvancementEvents.Loaded`, `Modify`, `Replace` (chargement des advancements, pas l'obtention).
- Méthode vanilla : `public boolean award(AdvancementHolder holder, String criterion)`, ligne 167 de `server/PlayerAdvancements.java`. Retourne `true` si le critère a progressé. Le passage à « terminé » est `AdvancementProgress.isDone()` avant/après.
  Usage : `player.getAdvancements().award(holder, "criterion_name");`
- Révocation : `public boolean revoke(AdvancementHolder advancement, String criterion)`, ligne 192. Pas de `revokeAll` : la commande `/advancement revoke` boucle sur les critères (`server/commands/AdvancementCommands.java`, lignes 335 et 343).
- Progression : `AdvancementProgress getOrStartProgress(AdvancementHolder)`, ligne 282.
- Lookup d'un advancement : passer par le `MinecraftServer` (`getAdvancements()`) : non vérifié dans ce passage.
- IDs vanilla (vérifiés, fichiers présents dans `data/minecraft/advancement/` de `minecraft-common-13d9796e56-26.3.jar`) :
  - `minecraft:story/mine_stone`
  - `minecraft:story/smelt_iron`
  - `minecraft:story/mine_diamond`
  - `minecraft:story/enter_the_nether`
  - `minecraft:nether/find_fortress`
  - `minecraft:nether/obtain_blaze_rod`
  - `minecraft:story/follow_ender_eye`
  - `minecraft:story/enter_the_end`
  - `minecraft:end/kill_dragon`

## 7. Téléportation

- `public @Nullable ServerPlayer teleport(TeleportTransition transition)` : ligne 1124 de `server/level/ServerPlayer.java`.
  Usage : `player.teleport(new TeleportTransition(targetLevel, pos, Vec3.ZERO, yRot, xRot, TeleportTransition.DO_NOTHING));`
- `TeleportTransition` : `public record TeleportTransition(ServerLevel newLevel, Vec3 position, Vec3 deltaMovement, float yRot, float xRot, boolean missingRespawnBlock, boolean asPassenger, Set<Relative> relatives, PostTeleportTransition postTeleportTransition)`, package `net.minecraft.world.level.portal`. Constructeur court : `(ServerLevel, Vec3, Vec3, float yRot, float xRot, PostTeleportTransition)`.
- `public void teleportTo(double x, double y, double z)` : ligne 1717 (même classe), téléportation dans le même monde.
- Entité générique : `Entity teleport(TeleportTransition)` (retour `Entity`, pont de `Entity`).

## 8. DataVersion

- Valeur : **5023** (champ `world_version` de `version.json` du jar 26.3, `"id": "26.3"`, `"protocol_version": 777`).
- Accès code : `SharedConstants.getCurrentVersion().dataVersion().version()` (`util/Util.java` ligne 318 l'utilise). `WorldVersion.dataVersion()` est déclaré dans `WorldVersion.java` ligne 9.

---

## Non trouvé

- Événement de soin Fabric (utiliser `LivingEntity.heal`).
- `RuntimeWorldConfig`, `RuntimeWorldHandle`, `asWorld`, `openTemporaryWorld`, `getOrOpenPersistentWorld` (noms inexistants en 0.8.4+26.3).
- Support Nether/End lié dans Fantasy.
- `StructureTemplate.readStructure` (utiliser `NbtIo.readCompressed` + `load`).
- `Entity.getTags()` (utiliser `entityTags()`).
- Événement « advancement obtenu » côté Fabric.
- Position exacte de `ALLOW_DEATH` par rapport à `checkTotemDeathProtection` (lire les `@At` de `LivingEntityMixin`).
