package fr.classroomrun.game;

import fr.classroomrun.ClassroomRun;
import fr.classroomrun.command.RunCommand;
import fr.classroomrun.config.ModConfig;
import fr.classroomrun.http.StatsHttpServer;
import fr.classroomrun.http.StatsSnapshot;
import fr.classroomrun.journal.JournalService;
import fr.classroomrun.mixin.EnderDragonFightAccessor;
import fr.classroomrun.net.DeathPayload;
import fr.classroomrun.net.RunStatePayload;
import fr.classroomrun.net.Split;
import fr.classroomrun.stats.RunClock;
import fr.classroomrun.stats.RunStats;
import fr.classroomrun.stats.StatsStore;
import fr.classroomrun.sync.TeamSync;
import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.advancements.AdvancementHolder;
import net.minecraft.advancements.AdvancementProgress;
import net.minecraft.core.BlockPos;
import net.minecraft.ChatFormatting;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.game.ClientboundSetSubtitleTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitleTextPacket;
import net.minecraft.network.protocol.game.ClientboundSetTitlesAnimationPacket;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.dimension.end.EnderDragonFight;
import net.minecraft.world.level.portal.TeleportTransition;
import net.minecraft.world.level.storage.LevelData;
import net.minecraft.world.phys.Vec3;
import org.jspecify.annotations.Nullable;

import java.io.IOException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.ThreadLocalRandom;

/**
 * Orchestrates the run on the server thread: world trio, classroom, collective death, reset, chrono, splits, stats.
 *
 * <p>Run life cycle: at server start a run is prepared (new worlds + classroom) but not started; {@code /run start}
 * starts the chrono. A split sequence ending with KILL_DRAGON stops the chrono. A collective death or
 * {@code /run reset} rebuilds a fresh trio and puts everybody back in the new classroom; the chrono state
 * (started or not) is kept by a reset.
 *
 * <p>Collective death: {@code ALLOW_DEATH} returns false (vanilla death cancelled, justification: no death screen, no
 * dropped inventory, no respawn logic to bend) after the victim is restored to full health; the run enters the
 * "ending" state during which all damage is refused and the chrono is paused. Members become spectators with a
 * "Vous êtes morts" title so everybody sees the death, then {@code deathResetDelayTicks} later the reset happens at
 * the end of a server tick and puts them back in survival.
 */
public final class RunManager {
	private static final String TOKEN_PREFIX = "classroomrun:run_";
	/** Member turned spectator by a collective death; persisted in player data so a reconnect after the reset restores survival. */
	private static final String GHOST_TAG = "classroomrun:ghost";
	private static @Nullable RunManager instance;

	private final MinecraftServer server;
	private final ModConfig config;
	private final Path statsFile;
	private final RunStats stats;
	private final RunClock clock;
	private final TeamSync team = new TeamSync();
	private final JournalService journal;
	private final List<ServerPlayer> joined = new ArrayList<>();
	private @Nullable StatsHttpServer http;
	private @Nullable RunWorlds worlds;
	private Classroom.@Nullable Placed classroom;
	/** False before /run start (waiting in the classroom: no damage, no split probing). */
	private boolean started;
	/** Incremented at each reset and each /run start; with the run number it identifies the "clean slate" of a player. */
	private int epoch;
	/** Chrono running and splits probed: started and not finished. */
	private boolean active;
	private int resetCountdown = -1;
	private long tickCount;
	private boolean dirty;

	private RunManager(MinecraftServer server) {
		this.server = server;
		Path dir = FabricLoader.getInstance().getConfigDir().resolve("classroomrun");
		this.config = ModConfig.load(dir.resolve("config.json"), ClassroomRun.LOGGER);
		this.statsFile = dir.resolve("stats.json");
		this.stats = StatsStore.load(this.statsFile, ClassroomRun.LOGGER);
		this.clock = new RunClock(this.stats.totalTimeMs);
		this.journal = new JournalService(server, dir.resolve("runs"), this.clock::totalTimeMs);
	}

	public static @Nullable RunManager get() {
		return instance;
	}

	/** Called once from the mod initializer. */
	public static void register() {
		TeamSync.registerEvents();
		ServerLifecycleEvents.SERVER_STARTED.register(server -> {
			instance = new RunManager(server);
			instance.start();
		});
		ServerLifecycleEvents.SERVER_STOPPING.register(server -> {
			if (instance != null) {
				instance.stop();
				instance = null;
			}
		});
		ServerTickEvents.END_SERVER_TICK.register(server -> {
			if (instance != null) {
				instance.tick();
			}
		});
		ServerPlayConnectionEvents.JOIN.register((handler, sender, server) -> {
			if (instance != null) {
				instance.joined.add(handler.getPlayer());
			}
		});
		CommandRegistrationCallback.EVENT.register((dispatcher, registry, environment) -> RunCommand.register(dispatcher));
	}

	public MinecraftServer server() {
		return this.server;
	}

	public TeamSync team() {
		return this.team;
	}

	public @Nullable RunWorlds worlds() {
		return this.worlds;
	}

	public JournalService journal() {
		return this.journal;
	}

	public RunStats stats() {
		return this.stats;
	}

	public boolean isActive() {
		return this.active;
	}

	/** False while waiting for /run start: damage is refused. */
	public boolean isStarted() {
		return this.started;
	}

	/** True between a collective death and the reset: damage refused, chrono paused. */
	public boolean isEnding() {
		return this.resetCountdown >= 0;
	}

	// ---------------------------------------------------------------- life cycle

	private void start() {
		this.stats.newRun();
		this.team.resetVitals();
		this.worlds = RunWorlds.open(this.server, ThreadLocalRandom.current().nextLong());
		this.classroom = Classroom.build(this.server, this.worlds.overworld());
		refreshBoard();
		save();
		this.journal.onRunPrepared(this.stats.runNumber, false);
		startHttp();
		publishSnapshot();
		ClassroomRun.LOGGER.info("Run #{} ready, waiting for /run start", this.stats.runNumber);
	}

	private void startHttp() {
		int port = this.config.httpPort;
		if (port == 0) {
			return;
		}
		if (port < 0 || port > 65535) {
			ClassroomRun.LOGGER.warn("Invalid httpPort {} in config.json (0..65535), stats endpoint disabled", port);
			return;
		}
		try {
			this.http = new StatsHttpServer(this.config.httpBind, port, this.journal.store());
			this.http.start();
			ClassroomRun.LOGGER.info("Stats endpoint listening on {}:{}", this.config.httpBind, port);
		} catch (IOException | RuntimeException e) {
			this.http = null;
			ClassroomRun.LOGGER.warn("Cannot start the stats endpoint on {}:{}: {}", this.config.httpBind, port, e.getMessage());
		}
	}

	private void stop() {
		this.journal.shutdown();
		this.stats.totalTimeMs = this.clock.totalTimeMs();
		save();
		if (this.http != null) {
			this.http.stop();
			this.http = null;
		}
	}

	/** {@code /run start}. @return false if the run was already started */
	public boolean startRun() {
		if (this.active) {
			return false;
		}
		this.started = true;
		this.active = true;
		this.dirty = true;
		this.epoch++;
		// everybody starts equipped with nothing, in the classroom, with no advancement
		for (ServerPlayer player : List.copyOf(this.server.getPlayerList().getPlayers())) {
			if (TeamSync.isMember(player)) {
				resetPlayer(player);
			} else {
				moveToClassroom(player);
			}
		}
		this.journal.onGo();
		broadcastState();
		return true;
	}

	/** Collective death entry point, called from {@code ALLOW_DEATH} (inside the hurt call: nothing is moved here). */
	public void onCollectiveDeath(ServerPlayer victim, DamageSource source) {
		if (isEnding()) {
			return;
		}
		this.resetCountdown = Math.max(0, this.config.deathResetDelayTicks);
		this.stats.recordDeath();
		this.stats.totalTimeMs = this.clock.totalTimeMs();
		// leave the dying state so vanilla death handling (hurtServer continues after the event) has nothing to do
		victim.setHealth(victim.getMaxHealth());
		Optional<String> attacker = Optional.ofNullable(source.getEntity()).map(e -> e instanceof ServerPlayer p
				? p.getGameProfile().name()
				: BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString());
		DeathPayload payload = new DeathPayload(victim.getUUID(),
				source.typeHolder().unwrapKey().map(k -> k.identifier().toString()).orElse(source.getMsgId()),
				victim.getCombatTracker().getDeathMessage().getString(), attacker, victim.blockPosition(),
				victim.level().dimension());
		this.journal.onDeath(victim, source, payload);
		sendAll(payload);
		showDeath(payload.message());
		refreshBoard();
		broadcastState();
		save();
		ClassroomRun.LOGGER.info("Collective death of the team ({}), reset in {} ticks", payload.message(), this.resetCountdown);
	}

	/** Everybody sees the death: members become spectators until the reset, with a title over the death message. */
	private void showDeath(String message) {
		int seconds = Math.max(1, Math.round(this.resetCountdown / 20.0F));
		for (ServerPlayer player : List.copyOf(this.server.getPlayerList().getPlayers())) {
			if (TeamSync.isMember(player)) {
				player.addTag(GHOST_TAG);
				player.setGameMode(GameType.SPECTATOR);
			}
			player.connection.send(new ClientboundSetTitlesAnimationPacket(10, Math.max(20, this.resetCountdown - 10), 10));
			player.connection.send(new ClientboundSetSubtitleTextPacket(Component.literal(message + " · nouvelle run dans " + seconds + " s").withStyle(ChatFormatting.GRAY)));
			player.connection.send(new ClientboundSetTitleTextPacket(Component.literal("Vous êtes morts").withStyle(ChatFormatting.RED)));
		}
	}

	/** Back to survival for a member made spectator by a collective death (called before any reset of the player). */
	private static void reviveGhost(ServerPlayer player) {
		if (player.entityTags().remove(GHOST_TAG)) {
			player.setGameMode(GameType.SURVIVAL);
		}
	}

	/** {@link #reset()} after zeroing every stat (chrono, deaths, run number, splits): the next run is #1. Server thread only. */
	public void wipe() {
		this.clock.clear();
		this.stats.wipe();
		this.active = false; // the fresh run waits for /run start
		reset();
	}

	/** Rebuilds the world trio and sends everybody back to the new classroom. Server thread only. */
	public void reset() {
		RunWorlds old = this.worlds;
		RunWorlds fresh = RunWorlds.open(this.server, ThreadLocalRandom.current().nextLong());
		Classroom.Placed placed;
		try {
			placed = Classroom.build(this.server, fresh.overworld());
		} catch (RuntimeException e) {
			fresh.delete();
			throw e;
		}
		this.journal.closeRun();
		this.worlds = fresh;
		this.classroom = placed;
		this.resetCountdown = -1;
		this.epoch++;
		if (!this.active) {
			this.started = false; // a finished (or never started) run waits for /run start again
		}
		this.stats.newRun();
		this.team.resetVitals();
		for (ServerPlayer player : List.copyOf(this.server.getPlayerList().getPlayers())) {
			reviveGhost(player);
			resetPlayer(player);
		}
		this.journal.onRunPrepared(this.stats.runNumber, this.started);
		if (old != null) {
			detachDragonBar(old);
			old.delete();
		}
		refreshBoard();
		broadcastState();
		save();
		publishSnapshot();
		ClassroomRun.LOGGER.info("Run #{} ready", this.stats.runNumber);
	}

	/** Teleports to the classroom spawn and wipes inventory, XP, effects, advancements; full health and food. */
	private String token() {
		return TOKEN_PREFIX + this.stats.runNumber + "_" + this.epoch;
	}

	private void resetPlayer(ServerPlayer player) {
		player.entityTags().removeIf(t -> t.startsWith(TOKEN_PREFIX));
		player.addTag(token());
		player.closeContainer(); // crafting grid and open container are returned before the wipe
		player.containerMenu.setCarried(ItemStack.EMPTY);
		moveToClassroom(player);
		player.getInventory().clearContent();
		player.getEnderChestInventory().clearContent();
		player.removeAllEffects();
		player.setExperienceLevels(0);
		player.setExperiencePoints(0);
		player.totalExperience = 0;
		revokeAdvancements(player);
		player.clearFire();
		player.resetFallDistance();
		player.setHealth(player.getMaxHealth());
		player.setAbsorptionAmount(0.0F);
		player.getFoodData().setFoodLevel(20);
		player.getFoodData().setSaturation(5.0F);
	}

	/** The dragon bar is ticked by the fight of the End level: without this it would stay frozen on the clients. */
	private static void detachDragonBar(RunWorlds old) {
		EnderDragonFight fight = old.end().getDragonFight();
		if (fight != null) {
			((EnderDragonFightAccessor) fight).classroomrun$getDragonEvent().removeAllPlayers();
		}
		// shortcut: raid boss bars are not detached; raids in the old Overworld are not expected in this mod.
	}

	private void revokeAdvancements(ServerPlayer player) {
		for (AdvancementHolder holder : this.server.getAdvancements().getAllAdvancements()) {
			AdvancementProgress progress = player.getAdvancements().getOrStartProgress(holder);
			for (String criterion : List.copyOf(toList(progress.getCompletedCriteria()))) {
				player.getAdvancements().revoke(holder, criterion);
			}
		}
	}

	private static List<String> toList(Iterable<String> it) {
		List<String> list = new ArrayList<>();
		it.forEach(list::add);
		return list;
	}

	private void moveToClassroom(ServerPlayer player) {
		Classroom.Placed room = this.classroom;
		if (room == null) {
			return;
		}
		player.teleport(new TeleportTransition(room.level(), room.spawn(), Vec3.ZERO, 0.0F, 0.0F, TeleportTransition.DO_NOTHING));
		player.setRespawnPosition(new ServerPlayer.RespawnConfig(
				LevelData.RespawnData.of(room.level().dimension(), BlockPos.containing(room.spawn()), 0.0F, 0.0F), true), false);
	}

	/** Transition to the classroom spawn (exit of the run End). */
	public TeleportTransition classroomTransition() {
		Classroom.Placed room = this.classroom;
		return new TeleportTransition(room.level(), room.spawn(), Vec3.ZERO, 0.0F, 0.0F, TeleportTransition.DO_NOTHING);
	}

	// ---------------------------------------------------------------- tick

	private void tick() {
		this.tickCount++;
		List<ServerPlayer> players = this.server.getPlayerList().getPlayers();
		boolean anyPlayer = !players.isEmpty();

		if (!this.joined.isEmpty()) {
			for (ServerPlayer player : this.joined) {
				onJoin(player);
			}
			this.joined.clear();
		}

		boolean running = this.active && !isEnding() && anyPlayer;
		this.clock.tick(System.nanoTime(), running);
		this.stats.totalTimeMs = this.clock.totalTimeMs();

		if (isEnding()) {
			if (--this.resetCountdown < 0) {
				this.resetCountdown = 0;
				reset();
			}
		} else if (anyPlayer) {
			this.team.tick(this.server);
			this.journal.sample(players);
			if (this.active && this.tickCount % 10 == 0) {
				probeSplits(players.stream().filter(TeamSync::isMember).toList());
			}
		}

		if (running && this.tickCount % 100 == 0) {
			broadcastState();
		}
		if (this.tickCount % 20 == 0) {
			publishSnapshot();
		}
		if ((this.dirty || running) && this.tickCount % 600 == 0) {
			save();
		}
	}

	private void onJoin(ServerPlayer player) {
		if (player.hasDisconnected()) {
			return;
		}
		if (!isEnding()) {
			reviveGhost(player); // offline during the end of a run: the reset already happened without them
		}
		if (TeamSync.isMember(player) && !player.entityTags().contains(token())) {
			resetPlayer(player); // offline during a reset or /run start: same clean slate as the team
		} else if (this.worlds != null && !this.worlds.contains(player.level())) {
			moveToClassroom(player); // creative/spectator: moved only
		}
		this.team.onJoin(this.server, player);
		refreshBoard();
		ServerPlayNetworking.send(player, statePayload());
	}

	private void probeSplits(List<ServerPlayer> players) {
		RunWorlds w = this.worlds;
		if (w == null || !this.active) {
			return;
		}
		int found = SplitProbe.probe(this.server, players, w, this.stats.reachedSplits);
		boolean any = false;
		for (Split split : Split.values()) {
			if ((found & split.bit()) != 0 && this.stats.markSplit(split)) {
				any = true;
				ClassroomRun.LOGGER.info("Split reached: {}", split);
				this.journal.onSplit(split);
			}
		}
		if (any) {
			if (this.stats.finished()) {
				this.active = false; // KILL_DRAGON reached, even if an earlier split was skipped
				this.journal.onEnd("dragon");
			}
			this.dirty = true;
			refreshBoard();
			broadcastState();
			save();
		}
	}

	// ---------------------------------------------------------------- state, board, persistence

	public RunStatePayload statePayload() {
		boolean running = this.active && !isEnding() && !this.server.getPlayerList().getPlayers().isEmpty();
		return new RunStatePayload(this.clock.totalTimeMs(), running, this.stats.deaths, this.stats.runNumber,
				this.stats.currentSplit(), this.stats.reachedSplits);
	}

	private void broadcastState() {
		sendAll(statePayload());
	}

	private void sendAll(CustomPacketPayload payload) {
		for (ServerPlayer p : this.server.getPlayerList().getPlayers()) {
			ServerPlayNetworking.send(p, payload);
		}
	}

	/** Board labels, indexed like {@link Split}. */
	private static final String[] SPLIT_LABELS = {"pioche", "lingot de fer", "diamant", "portail du Nether",
			"verrue du Nether", "bâton de blaze", "œil de l'Ender", "entrée dans l'End", "Ender Dragon tué"};

	private void refreshBoard() {
		int bestIndex = Math.max(-1, Math.min(Split.values().length - 1, this.stats.bestSplit));
		String best = bestIndex >= 0 ? SPLIT_LABELS[bestIndex] : "aucun";
		int deaths = this.stats.deaths;
		Classroom.updateBoard(this.classroom, "Run #" + this.stats.runNumber + "\n" + deaths + (deaths > 1 ? " morts" : " mort")
				+ "\nMeilleur split : " + best);
	}

	private void publishSnapshot() {
		StatsHttpServer h = this.http;
		if (h != null) {
			h.publish(new StatsSnapshot(this.stats.runNumber, this.clock.totalTimeMs(), this.stats.deaths,
					this.stats.currentSplit(), this.stats.reachedSplits, this.stats.bestSplit,
					this.server.getPlayerList().getPlayers().stream().map(p -> p.getGameProfile().name()).toList(),
					this.active && !isEnding()));
		}
	}

	private void save() {
		this.dirty = false;
		this.stats.totalTimeMs = this.clock.totalTimeMs();
		try {
			StatsStore.save(this.statsFile, this.stats);
		} catch (IOException e) {
			ClassroomRun.LOGGER.warn("Cannot save {}: {}", this.statsFile, e.getMessage());
		}
	}

	/** Used by {@code /run stats}. */
	public String describe() {
		Split[] splits = Split.values();
		int current = this.stats.currentSplit();
		return "Run #" + this.stats.runNumber + (this.active ? " (en cours)" : " (arretee)")
				+ ", chrono " + formatTime(this.clock.totalTimeMs()) + ", " + this.stats.deaths + " morts, prochain split : "
				+ (current < splits.length ? splits[current].name() : "termine")
				+ ", meilleur split : " + (this.stats.bestSplit >= 0 ? splits[this.stats.bestSplit].name() : "aucun");
	}

	private static String formatTime(long ms) {
		long s = ms / 1000;
		return String.format("%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60);
	}
}
