# API Minecraft 26.3 / Fabric API 0.165.0+26.3 : réseau, HUD, tête de joueur, commandes client, temps

Sources :
- Fabric API 0.165.0+26.3 : `~/.gradle/caches/modules-2/files-2.1/net.fabricmc.fabric-api/fabric-api/0.165.0+26.3/.../fabric-api-0.165.0+26.3.pom` (versions des modules). Modules utilisés : `fabric-networking-api-v1` 6.4.0, `fabric-rendering-v1` 27.2.0, `fabric-command-api-v2` 3.1.2, `fabric-key-mapping-api-v1` 2.0.8. Aucun sources.jar pour ces trois modules : signatures obtenues par `javap` (JDK Temurin 25) sur les jars.
- Minecraft 26.3 : `D:\mod_hardcore\.gradle\loom-cache\minecraftMaven\net\minecraft\minecraft-clientOnly-13d9796e56\26.3\minecraft-clientOnly-13d9796e56-26.3-sources.jar` et `minecraft-common-13d9796e56\26.3\minecraft-common-13d9796e56-26.3-sources.jar` (sources décompilées).

Abréviations de source : `[FA-net]` = fabric-networking-api-v1 6.4.0 (javap), `[FA-rend]` = fabric-rendering-v1 27.2.0 (javap), `[FA-cmd]` = fabric-command-api-v2 3.1.2 (javap), `[MC-cl]` = minecraft-clientOnly sources, `[MC-co]` = minecraft-common sources.

---

## 1. Réseau custom serveur vers client

### 1.1 Interface du payload et type
- `net.minecraft.network.protocol.common.custom.CustomPacketPayload` (interface) : `CustomPacketPayload.Type<? extends CustomPacketPayload> type();` [MC-co]
- `CustomPacketPayload.Type<T extends CustomPacketPayload>` est un `record Type<T>(Identifier id)`. [MC-co `CustomPacketPayload.java`]
- Exemple : `public record MonPayload(long x) implements CustomPacketPayload { public static final CustomPacketPayload.Type<MonPayload> TYPE = new CustomPacketPayload.Type<>(Identifier.fromNamespaceAndPath("classroomrun", "mon_payload")); @Override public CustomPacketPayload.Type<? extends CustomPacketPayload> type() { return TYPE; } }`
- `CustomPacketPayload.createType(String id)` existe mais force le namespace `minecraft` : ne pas l'utiliser pour un mod.

### 1.2 Codecs (`net.minecraft.network.codec.ByteBufCodecs`, `StreamCodec` : `net.minecraft.network.codec.StreamCodec`) [MC-co]
Tous sont de type `StreamCodec<ByteBuf, X>`.
- `ByteBufCodecs.LONG` : `StreamCodec<ByteBuf, Long>` (8 octets). `ByteBufCodecs.VAR_LONG` : variable.
- `ByteBufCodecs.INT` : `StreamCodec<ByteBuf, Integer>`. `ByteBufCodecs.VAR_INT` : variable.
- `ByteBufCodecs.FLOAT` : `StreamCodec<ByteBuf, Float>`. `ByteBufCodecs.DOUBLE` : `StreamCodec<ByteBuf, Double>`.
- `ByteBufCodecs.BOOL` : `StreamCodec<ByteBuf, Boolean>`.
- `ByteBufCodecs.STRING_UTF8` : `StreamCodec<ByteBuf, String>` (max 32767). `ByteBufCodecs.stringUtf8(int maxLength)` pour une limite.
- `ByteBufCodecs.optional(StreamCodec<? super B, V>)` : `StreamCodec<B, Optional<V>>`.
- `ByteBufCodecs.list()` et `ByteBufCodecs.list(int maxSize)` : `StreamCodec.CodecOperation<B, V, List<V>>`, à appliquer avec `.apply(...)`. `ByteBufCodecs.collection(IntFunction<C>)` idem pour `Collection`.
- UUID : `net.minecraft.core.UUIDUtil.STREAM_CODEC` : `StreamCodec<ByteBuf, UUID>`.
- BlockPos : `net.minecraft.core.BlockPos.STREAM_CODEC` : `StreamCodec<ByteBuf, BlockPos>`.
- Vec3 : `net.minecraft.world.phys.Vec3.STREAM_CODEC` : `StreamCodec<ByteBuf, Vec3>` (double). `Vec3.LP_STREAM_CODEC` : version compressée.
- Identifier : `net.minecraft.resources.Identifier.STREAM_CODEC` : `StreamCodec<ByteBuf, Identifier>`.
- ResourceKey de dimension : `net.minecraft.resources.ResourceKey.streamCodec(Registries.DIMENSION)` : `StreamCodec<ByteBuf, ResourceKey<T>>` (T = `Level`). `Registries.DIMENSION` : `net.minecraft.core.registries.Registries.DIMENSION` : `ResourceKey<Registry<Level>>`. Codec de la clé registre : `ResourceKey.REGISTRY_STREAM_CODEC`.
- Composition (record) : `StreamCodec.composite(codec1, MonPayload::x, codec2, MonPayload::y, MonPayload::new)`, jusqu'à 10 champs [MC-co `StreamCodec.java`]. Exemple : `public static final StreamCodec<ByteBuf, MonPayload> CODEC = StreamCodec.composite(ByteBufCodecs.LONG, MonPayload::x, ByteBufCodecs.FLOAT, MonPayload::f, MonPayload::new);`

### 1.3 Enregistrement du type (S2C en play)
- `net.fabricmc.fabric.api.networking.v1.PayloadTypeRegistry<B>` [FA-net] : méthodes statiques `clientboundPlay()` et `serverboundPlay()` (`PayloadTypeRegistry<RegistryFriendlyByteBuf>`), `clientboundConfiguration()`, `serverboundConfiguration()`.
- Signature : `<T extends CustomPacketPayload> CustomPacketPayload.TypeAndCodec<? super B, T> register(CustomPacketPayload.Type<T> type, StreamCodec<? super B, T> codec)`.
- Exemple (commun, avant tout envoi) : `PayloadTypeRegistry.clientboundPlay().register(MonPayload.TYPE, MonPayload.CODEC);`
- Variante `registerLarge(type, codec, int maxSize)` pour les gros payloads.

### 1.4 Envoi serveur
- `net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking` [FA-net] : `public static void send(ServerPlayer player, CustomPacketPayload payload)`.
- Exemple : `ServerPlayNetworking.send(player, new MonPayload(42L));`
- Test préalable : `ServerPlayNetworking.canSend(ServerPlayer, CustomPacketPayload.Type<?>)` : boolean.

### 1.5 Réception client
- `net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking` [FA-net] : `public static <T extends CustomPacketPayload> boolean registerGlobalReceiver(CustomPacketPayload.Type<T> type, ClientPlayNetworking.PlayPayloadHandler<T> handler)`.
- Interface fonctionnelle : `ClientPlayNetworking.PlayPayloadHandler<T extends CustomPacketPayload>` : `void receive(T payload, ClientPlayNetworking.Context context)`.
- `ClientPlayNetworking.Context` : `Minecraft client()`, `LocalPlayer player()`, `PacketSender responseSender()`, `PacketContext packetContext()` (default).
- Exemple : `ClientPlayNetworking.registerGlobalReceiver(MonPayload.TYPE, (payload, context) -> { ... });`
- Thread : [FA-net, bytecode de `ClientCommonPacketListenerImplMixin`] le handler est planifié via `Minecraft.packetProcessor().scheduleIfPossible(listener, packet)`. Il s'exécute donc sur le thread principal du client, pas sur le thread netty. Accès direct à `client().level` et au HUD sans `client().execute(...)`. Non vérifié en jeu.
- Le type S2C doit être enregistré côté client et serveur (section 1.3).

### 1.6 Identifier
- Classe : `net.minecraft.resources.Identifier` (renommée de ResourceLocation) [MC-co `Identifier.java`].
- Fabrique : `public static Identifier fromNamespaceAndPath(String namespace, String path)` [MC-co l.40].
- Exemple : `Identifier.fromNamespaceAndPath("classroomrun", "mon_payload")`.
- Autres : `Identifier.parse(String)`, `Identifier.withDefaultNamespace(String path)` (namespace `minecraft`), `Identifier.tryBuild(ns, path)` (nullable).

---

## 2. HUD client

### 2.1 HudRenderCallback
- `HudRenderCallback` est ABSENT du module `fabric-rendering-v1` 27.2.0 (aucune classe de ce nom dans le jar). NON TROUVÉ : ne pas l'utiliser.

### 2.2 HudElementRegistry (remplacement)
- Package : `net.fabricmc.fabric.api.client.rendering.v1.hud` [FA-rend].
- Interface fonctionnelle `HudElement` : `void extractRenderState(net.minecraft.client.gui.GuiGraphicsExtractor graphics, net.minecraft.client.DeltaTracker deltaTracker)`.
- `HudElementRegistry` (interface, méthodes statiques) :
  - `static void addFirst(Identifier id, HudElement element)`
  - `static void addLast(Identifier id, HudElement element)`
  - `static void attachElementBefore(Identifier pivotId, Identifier id, HudElement element)`
  - `static void attachElementAfter(Identifier pivotId, Identifier id, HudElement element)`
  - `static void removeElement(Identifier id)`
  - `static void replaceElement(Identifier id, Function<HudElement, HudElement> replacer)`
- Exemple : `HudElementRegistry.attachElementAfter(VanillaHudElements.HEALTH_BAR, Identifier.fromNamespaceAndPath("classroomrun", "hud_run"), (graphics, delta) -> { ... });`
- Sémantique exacte "before/after" (ordre de dessin) : déduite du nom, non vérifiée dans le code.

### 2.3 VanillaHudElements (`net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements`) [FA-rend]
Constantes `Identifier` publiques : `MISC_OVERLAYS, CROSSHAIR, SPECTATOR_MENU, HOTBAR, ARMOR_BAR, HEALTH_BAR, FOOD_BAR, AIR_BAR, MOUNT_HEALTH, INFO_BAR, EXPERIENCE_LEVEL, INFO_BAR_FOREGROUND, HELD_ITEM_TOOLTIP, SPECTATOR_TOOLTIP, MOB_EFFECTS, BOSS_BAR, SLEEP, DEMO_TIMER, SCOREBOARD, OVERLAY_MESSAGE, TITLE_AND_SUBTITLE, CHAT, PLAYER_LIST, SUBTITLES`.
Près de la barre de vie : `HEALTH_BAR`, `ARMOR_BAR`, `FOOD_BAR`.

### 2.4 GuiGraphicsExtractor (et non GuiGraphics)
- Classe : `net.minecraft.client.gui.GuiGraphicsExtractor` [MC-cl]. C'est le type reçu par `HudElement`.
- Item : `void item(ItemStack itemStack, int x, int y)` ; variante `item(ItemStack, int x, int y, int seed)`. Nombre : `itemDecorations(Font font, ItemStack itemStack, int x, int y)`.
- Texte : `void text(Font font, @Nullable String str, int x, int y, int color)` et `text(Font, String, int x, int y, int color, boolean dropShadow)`. Pas de `drawString`. Centré : `centeredText(Font, String, int x, int y, int color)`.
- Rectangle plein : `void fill(int x0, int y0, int x1, int y1, int col)`. Variante pipeline : `fill(RenderPipeline, int x0, int y0, int x1, int y1, int col)`.
- Texture : `void blit(RenderPipeline renderPipeline, Identifier texture, int x, int y, float u, float v, int width, int height, int textureWidth, int textureHeight, int color)` (pipeline `RenderPipelines.GUI_TEXTURED`, `net.minecraft.client.renderer.RenderPipelines`).
- Sprite : `void blitSprite(RenderPipeline renderPipeline, Identifier location, int x, int y, int width, int height)`.
- Police : `Minecraft.getInstance().font` (type `net.minecraft.client.gui.Font`). Nom du getter non vérifié par grep : à confirmer.
- Classe `Minecraft` : `net.minecraft.client.Minecraft`, singleton `Minecraft.getInstance()`.

---

## 3. Tête de joueur (liste Tab) et skin par UUID

### 3.1 Rendu de tête
- Classe : `net.minecraft.client.gui.components.PlayerFaceExtractor` [MC-cl] (remplace PlayerFaceRenderer, absent en 26.3).
- Signatures statiques :
  - `void extractRenderState(GuiGraphicsExtractor graphics, PlayerSkin skin, int x, int y, int size)`
  - `void extractRenderState(GuiGraphicsExtractor graphics, PlayerSkin skin, int x, int y, int size, int color)`
  - `void extractRenderState(GuiGraphicsExtractor graphics, ResolvableProfile skinProfile, int x, int y, int size)`
  - `void extractRenderState(GuiGraphicsExtractor graphics, Identifier texture, int x, int y, int size, boolean hat, boolean flip, int color)`
- Exemple : `PlayerFaceExtractor.extractRenderState(graphics, playerInfo.getSkin(), x, y, 8);`
- `PlayerSkin` : `net.minecraft.world.entity.player.PlayerSkin` [MC-co], `record PlayerSkin(ClientAsset.Texture body, ClientAsset.Texture cape, ClientAsset.Texture elytra, PlayerModelType model, boolean secure)`.

### 3.2 Skin d'un UUID côté client
- `net.minecraft.client.multiplayer.ClientPacketListener` [MC-cl]. Accès : `Minecraft.getInstance().getConnection()` (nullable hors connexion).
- `@Nullable PlayerInfo getPlayerInfo(UUID player)` ; aussi `getPlayerInfo(String)` et `getPlayerInfoIgnoreCase(String)`.
- `PlayerInfo` : `net.minecraft.client.multiplayer.PlayerInfo` [MC-cl] : `GameProfile getProfile()` ; `PlayerSkin getSkin()`.
- Exemple : `var conn = Minecraft.getInstance().getConnection(); PlayerInfo info = conn == null ? null : conn.getPlayerInfo(uuid); if (info != null) PlayerFaceExtractor.extractRenderState(g, info.getSkin(), x, y, 8);`
- `Minecraft.getInstance().playerSkinRenderCache()` (`net.minecraft.client.renderer.PlayerSkinRenderCache`, `getOrDefault(ResolvableProfile)`) sert aux profils absents de la liste. Non utilisé ici.

---

## 4. Commandes client Fabric

- Événement : `net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback` [FA-cmd] :
  - `Event<ClientCommandRegistrationCallback> EVENT` (static final)
  - `void register(CommandDispatcher<FabricClientCommandSource> dispatcher, CommandBuildContext registryAccess)`
- Constructeurs : `net.fabricmc.fabric.api.client.command.v2.ClientCommands` [FA-cmd] :
  - `static LiteralArgumentBuilder<FabricClientCommandSource> literal(String name)`
  - `static <T> RequiredArgumentBuilder<FabricClientCommandSource, T> argument(String name, ArgumentType<T> type)`
  - `static CommandDispatcher<FabricClientCommandSource> getActiveDispatcher()`
- Source : `net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource` [FA-cmd] : `void sendFeedback(Component)`, `void sendError(Component)`, `Minecraft getClient()`, `LocalPlayer getPlayer()`, `ClientLevel getLevel()`, `boolean attended()`.
- Exemple : `ClientCommandRegistrationCallback.EVENT.register((dispatcher, access) -> dispatcher.register(ClientCommands.literal("cr").executes(c -> { c.getSource().sendFeedback(Component.literal("ok")); return 1; })));`
- Brigadier : `com.mojang.brigadier.CommandDispatcher`, `com.mojang.brigadier.builder.LiteralArgumentBuilder`. Texte : `net.minecraft.network.chat.Component`.

---

## 5. Temps réel côté client

- `net.minecraft.util.Util` [MC-co] : `public static long getMillis()` (ms, monotone), `public static long getNanos()`.
- Exemple : `long now = Util.getMillis();`
- Ne pas utiliser `System.currentTimeMillis()` pour un chrono de jeu (horloge murale). `DeltaTracker` (`net.minecraft.client.DeltaTracker`) donne le delta de frame en HUD.

---

## Non trouvé / à vérifier
- NON TROUVÉ : `HudRenderCallback` (absent de fabric-rendering-v1 27.2.0).
- NON TROUVÉ : `PlayerFaceRenderer` (remplacé par `PlayerFaceExtractor`).
- NON TROUVÉ : `GuiGraphics` 26.3 (remplacé par `GuiGraphicsExtractor`).
- À VÉRIFIER : sémantique d'ordre de `attachElementBefore/After` ; nom du getter de la police ; thread d'exécution déduit du bytecode, non testé en jeu.
- Sources absentes pour fabric-networking-api-v1 6.4.0, fabric-rendering-v1 27.2.0, fabric-command-api-v2 3.1.2 : signatures issues de `javap` uniquement (pas de Javadoc).
