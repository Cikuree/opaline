package fr.classroomrun.journal;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import fr.classroomrun.ClassroomRun;
import fr.classroomrun.game.RunManager;
import fr.classroomrun.net.DeathPayload;
import fr.classroomrun.net.HistoryPayload;
import fr.classroomrun.net.HistoryRequestPayload;
import fr.classroomrun.net.Split;
import fr.classroomrun.sync.TeamSync;
import net.fabricmc.fabric.api.networking.v1.ServerPlayConnectionEvents;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.util.Util;
import net.minecraft.world.damagesource.DamageSource;
import org.jspecify.annotations.Nullable;

import java.io.IOException;
import java.nio.file.Path;
import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.function.LongSupplier;

/**
 * Minecraft glue of the death history: the per-run journal ({@code runs/<n>/journal.jsonl}), the 10 s health buffer,
 * and the answers to {@link HistoryRequestPayload}. All {@code on*} methods run on
 * the server thread.
 */
public final class JournalService {
	private final MinecraftServer server;
	private final JournalStore store;
	private final LongSupplier chronoMs;
	private final HealthBuffer buffer = new HealthBuffer();
	private final RateLimiter limiter = new RateLimiter(1000);
	private final ExecutorService io = Executors.newSingleThreadExecutor(r -> {
		Thread t = new Thread(r, "classroomrun-history");
		t.setDaemon(true);
		return t;
	});
	private @Nullable RunJournal journal;
	private int run;
	private boolean started;
	private boolean ended;

	public JournalService(MinecraftServer server, Path runsDir, LongSupplier chronoMs) {
		this.server = server;
		this.store = new JournalStore(runsDir);
		this.chronoMs = chronoMs;
	}

	// ---------------------------------------------------------------- registration

	/** Called once from the mod initializer. */
	public static void register() {
		ServerPlayNetworking.registerGlobalReceiver(HistoryRequestPayload.TYPE, (payload, context) -> {
			RunManager run = RunManager.get();
			if (run != null) {
				run.journal().onRequest(context.player(), payload.runNumber());
			}
		});
		ServerPlayConnectionEvents.DISCONNECT.register((handler, server) -> {
			RunManager run = RunManager.get();
			if (run != null) {
				run.journal().onLeave(handler.getPlayer());
			}
		});
	}

	// ---------------------------------------------------------------- run life cycle

	/** A new run exists (worlds and classroom ready). {@code started}: the chrono is already running (reset mid-run). */
	public void onRunPrepared(int runNumber, boolean started) {
		closeJournal();
		this.run = runNumber;
		this.started = false;
		this.ended = false;
		this.buffer.clear();
		try {
			this.journal = new RunJournal(this.store.journalFile(runNumber));
			ClassroomRun.LOGGER.info("Journal opened: {}", this.store.journalFile(runNumber));
		} catch (IOException e) {
			this.journal = null;
			ClassroomRun.LOGGER.warn("Cannot open the journal of run #{}: {}", runNumber, e.getMessage());
			return;
		}
		append(JournalEvents.event(JournalEvents.START, Instant.now(), this.chronoMs.getAsLong()));
		if (started) {
			onGo();
		}
	}

	/** {@code /run start}: the chrono runs. */
	public void onGo() {
		if (this.started || this.journal == null) {
			return;
		}
		this.started = true;
		append(JournalEvents.event(JournalEvents.GO, Instant.now(), this.chronoMs.getAsLong()));
	}

	/** Writes the end line once (reasons: death, dragon, reset, stop). */
	public void onEnd(String reason) {
		if (this.journal == null || this.ended) {
			return;
		}
		this.ended = true;
		JsonObject o = JournalEvents.event(JournalEvents.END, Instant.now(), this.chronoMs.getAsLong());
		o.addProperty("reason", reason);
		append(o);
	}

	/** The run is over and its worlds are about to be deleted. */
	public void closeRun() {
		onEnd("reset");
	}

	/** Server stopping: ends the run and closes the file. */
	public void shutdown() {
		onEnd("stop");
		closeJournal();
		this.io.shutdown();
	}

	private void closeJournal() {
		if (this.journal != null) {
			try {
				this.journal.close();
				ClassroomRun.LOGGER.info("Journal closed: run #{}", this.run);
			} catch (IOException e) {
				ClassroomRun.LOGGER.warn("Cannot close the journal: {}", e.getMessage());
			}
			this.journal = null;
		}
	}

	// ---------------------------------------------------------------- events

	/** Every tick: health sample of each member (the buffer keeps changes only). */
	public void sample(List<ServerPlayer> players) {
		if (!this.started || this.ended) {
			return;
		}
		long now = Util.getMillis();
		for (ServerPlayer p : players) {
			if (TeamSync.isMember(p)) {
				this.buffer.record(p.getUUID(), p.getGameProfile().name(), now, p.getHealth());
			}
		}
	}

	public void onDamage(ServerPlayer player, DamageSource source, float taken) {
		if (!this.started || this.ended) {
			return;
		}
		JsonObject o = JournalEvents.event(JournalEvents.DAMAGE, Instant.now(), this.chronoMs.getAsLong());
		who(o, player);
		o.addProperty("amount", taken);
		o.addProperty("cause", cause(source));
		o.addProperty("attacker", attacker(source));
		where(o, player);
		teamAfter(o, player);
		append(o);
	}

	public void onHeal(ServerPlayer player, float gained) {
		if (!this.started || this.ended) {
			return;
		}
		JsonObject o = JournalEvents.event(JournalEvents.HEAL, Instant.now(), this.chronoMs.getAsLong());
		who(o, player);
		o.addProperty("amount", gained);
		where(o, player);
		teamAfter(o, player);
		append(o);
	}

	/**
	 * The collective death. {@code fatalBy} is the player whose hit killed the team: the attacking player if there is
	 * one, otherwise the victim itself (fall, fire, mob...). Call before the victim's health is restored.
	 */
	public void onDeath(ServerPlayer victim, DamageSource source, DeathPayload payload) {
		if (this.journal == null || this.ended) {
			return;
		}
		String name = victim.getGameProfile().name();
		JsonObject o = JournalEvents.event(JournalEvents.DEATH, Instant.now(), this.chronoMs.getAsLong());
		o.addProperty("player", payload.player().toString());
		o.addProperty("name", name);
		o.addProperty("cause", payload.cause());
		o.addProperty("message", payload.message());
		o.addProperty("attacker", payload.attacker().orElse(null));
		o.addProperty("fatalBy", source.getEntity() instanceof ServerPlayer p ? p.getGameProfile().name() : name);
		BlockPos pos = payload.pos();
		o.add("pos", JournalEvents.pos(pos.getX(), pos.getY(), pos.getZ()));
		o.addProperty("dimension", payload.dimension().identifier().toString());
		// everybody shares the life: the whole team is at 0 at the death
		o.add("health", JournalEvents.health(this.buffer.snapshot(Util.getMillis(), 0.0F)));
		append(o);
		onEnd("death");
	}

	public void onSplit(Split split) {
		if (this.journal == null || this.ended) {
			return;
		}
		JsonObject o = JournalEvents.event(JournalEvents.SPLIT, Instant.now(), this.chronoMs.getAsLong());
		o.addProperty("split", split.name());
		append(o);
	}

	private void onLeave(ServerPlayer player) {
		this.buffer.remove(player.getUUID());
		this.limiter.forget(player.getUUID());
	}

	// ---------------------------------------------------------------- history requests

	private void onRequest(ServerPlayer player, int runNumber) {
		if (runNumber < HistoryRequestPayload.INDEX || !this.limiter.tryAcquire(player.getUUID(), Util.getMillis())) {
			return;
		}
		this.io.execute(() -> {
			String json;
			if (runNumber == HistoryRequestPayload.INDEX) {
				json = HistoryJson.index(this.store.summaries(), HistoryPayload.MAX_JSON);
			} else {
				JournalReader.Parsed p = this.store.run(runNumber);
				json = p == null ? HistoryJson.detail(runNumber, "", List.of(), HistoryPayload.MAX_JSON)
						: HistoryJson.detail(runNumber, p.summary().startedAt(), p.deaths(), HistoryPayload.MAX_JSON);
			}
			HistoryPayload payload = new HistoryPayload(runNumber, json);
			this.server.execute(() -> {
				if (!player.hasDisconnected()) {
					ServerPlayNetworking.send(player, payload);
				}
			});
		});
	}

	// ---------------------------------------------------------------- helpers

	private void append(JsonObject event) {
		RunJournal j = this.journal;
		if (j == null) {
			return;
		}
		try {
			j.append(event);
		} catch (IOException e) {
			ClassroomRun.LOGGER.warn("Cannot write the journal of run #{}: {}", this.run, e.getMessage());
		}
	}

	private static void who(JsonObject o, ServerPlayer p) {
		o.addProperty("player", p.getUUID().toString());
		o.addProperty("name", p.getGameProfile().name());
	}

	private static void where(JsonObject o, ServerPlayer p) {
		BlockPos pos = p.blockPosition();
		o.add("pos", JournalEvents.pos(pos.getX(), pos.getY(), pos.getZ()));
		o.addProperty("dimension", p.level().dimension().identifier().toString());
	}

	/** Life is shared, so after a hit or a heal the whole team is at the value of the player concerned. */
	private void teamAfter(JsonObject o, ServerPlayer player) {
		o.addProperty("healthAfter", player.getHealth());
		JsonArray team = new JsonArray();
		for (ServerPlayer p : this.server.getPlayerList().getPlayers()) {
			if (TeamSync.isSynced(p)) {
				team.add(p.getGameProfile().name());
			}
		}
		o.add("team", team);
	}

	private static String cause(DamageSource source) {
		return source.typeHolder().unwrapKey().map(k -> k.identifier().toString()).orElse(source.getMsgId());
	}

	private static @Nullable String attacker(DamageSource source) {
		return Optional.ofNullable(source.getEntity()).map(e -> e instanceof ServerPlayer p
				? p.getGameProfile().name()
				: BuiltInRegistries.ENTITY_TYPE.getKey(e.getType()).toString()).orElse(null);
	}
}
