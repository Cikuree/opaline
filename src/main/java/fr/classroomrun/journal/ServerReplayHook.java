package fr.classroomrun.journal;

import fr.classroomrun.ClassroomRun;
import me.senseiwells.replay.ServerReplay;
import net.casual.arcade.replay.io.ReplayFormat;
import net.casual.arcade.replay.recorder.ReplayRecorder;
import net.casual.arcade.replay.recorder.player.ReplayPlayerRecorder;
import net.casual.arcade.replay.recorder.player.ReplayPlayerRecorders;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.server.level.ServerPlayer;
import org.jspecify.annotations.Nullable;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

/**
 * ServerReplay (arcade-replay) side: one {@link ReplayPlayerRecorder} per player and per run, in
 * {@code <game dir>/recordings/classroomrun/run_<n>/<uuid>/<date>.mcpr}. Every ServerReplay reference of the mod is in
 * this class, loaded only if {@code server-replay} is present (see {@link JournalService}).
 *
 * <p>Only the recorders created here are managed; a recorder that ServerReplay itself started for a player
 * (its own "automatically_record" config) is left alone and gets no journal reference.
 */
final class ServerReplayHook implements ReplayHook {
	private static final int MARKER_COLOR = 0xFF5555;
	private static final Path RECORDINGS = FabricLoader.getInstance().getGameDir().resolve("recordings");
	/**
	 * {@code ReplayRecorder.getRecordingLength()} returns a Kotlin {@code Duration}, an inline class: the JVM name is
	 * mangled with a hash and cannot be written in Java, hence the method handle. Null if a ServerReplay update renames it.
	 */
	private static final @Nullable MethodHandle LENGTH = lengthHandle();

	private final Map<UUID, ReplayPlayerRecorder> recorders = new HashMap<>();
	private final Map<UUID, Long> startNanos = new HashMap<>();
	private final List<CompletableFuture<Long>> saving = new ArrayList<>();

	private static @Nullable MethodHandle lengthHandle() {
		try {
			return MethodHandles.publicLookup().findVirtual(ReplayRecorder.class, "getRecordingLength-UwyO8pc",
					MethodType.methodType(long.class));
		} catch (ReflectiveOperationException e) {
			ClassroomRun.LOGGER.warn("ServerReplay: getRecordingLength not found, replay offsets use the wall clock");
			return null;
		}
	}

	@Override
	public void start(ServerPlayer player, int run) {
		UUID id = player.getUUID();
		if (this.recorders.containsKey(id) || ReplayPlayerRecorders.has(id, ReplayFormat.ReplayMod)) {
			return;
		}
		try {
			Path dir = RECORDINGS.resolve("classroomrun").resolve("run_" + run).resolve(id.toString());
			ReplayPlayerRecorder recorder = ReplayPlayerRecorders.create(player, dir, ReplayFormat.ReplayMod,
					ServerReplay.getConfig().createSettings());
			if (recorder.start(ReplayRecorder.StartingMode.Start)) {
				this.recorders.put(id, recorder);
				this.startNanos.put(id, System.nanoTime());
			}
		} catch (RuntimeException e) {
			ClassroomRun.LOGGER.warn("Cannot start the replay recording of {}: {}", player.getGameProfile().name(), e.toString());
		}
	}

	@Override
	public void stop(ServerPlayer player) {
		stop(player.getUUID());
	}

	private void stop(UUID id) {
		ReplayPlayerRecorder recorder = this.recorders.remove(id);
		this.startNanos.remove(id);
		if (recorder == null || recorder.getStopped()) {
			return; // already stopped (ServerReplay closes the recorder of a player who disconnects)
		}
		try {
			this.saving.add(recorder.stop(true));
		} catch (RuntimeException e) {
			ClassroomRun.LOGGER.warn("Cannot stop the replay recording {}: {}", recorder.getName(), e.toString());
		}
	}

	@Override
	public void stopAll() {
		for (UUID id : List.copyOf(this.recorders.keySet())) {
			stop(id);
		}
	}

	@Override
	public void awaitSaved(long timeoutMs) {
		long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeoutMs);
		for (CompletableFuture<Long> f : this.saving) {
			try {
				f.get(Math.max(1L, deadline - System.nanoTime()), TimeUnit.NANOSECONDS);
			} catch (Exception e) {
				ClassroomRun.LOGGER.warn("A replay did not finish saving: {}", e.toString());
			}
		}
		this.saving.clear();
	}

	@Override
	public @Nullable Ref marker(String label, @Nullable ServerPlayer victim) {
		Ref ref = null;
		for (Map.Entry<UUID, ReplayPlayerRecorder> e : this.recorders.entrySet()) {
			ReplayPlayerRecorder recorder = e.getValue();
			if (recorder.getStopped()) {
				continue;
			}
			try {
				recorder.addMarker(label, MARKER_COLOR);
				if (victim != null && victim.getUUID().equals(e.getKey())) {
					ref = new Ref(relative(recorder), lengthMs(recorder, e.getKey()));
				}
			} catch (RuntimeException ex) {
				ClassroomRun.LOGGER.warn("Cannot add a replay marker to {}: {}", recorder.getName(), ex.toString());
			}
		}
		return ref;
	}

	/** The file this recorder produces: {@code location} is the working path, the saved file adds {@code .mcpr}. */
	private static String relative(ReplayRecorder recorder) {
		Path location = recorder.getLocation();
		return RECORDINGS.toAbsolutePath().normalize().relativize(location.toAbsolutePath().normalize())
				.toString().replace('\\', '/') + ".mcpr";
	}

	private long lengthMs(ReplayRecorder recorder, UUID id) {
		if (LENGTH != null) {
			try {
				long raw = (long) LENGTH.invokeExact(recorder);
				// kotlin.time.Duration storage: value << 1 | unit, unit 0 = nanoseconds, 1 = milliseconds
				long value = raw >> 1;
				return (raw & 1L) == 0L ? value / 1_000_000L : value;
			} catch (Throwable t) {
				ClassroomRun.LOGGER.warn("ServerReplay: cannot read the recording length: {}", t.toString());
			}
		}
		return TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - this.startNanos.getOrDefault(id, System.nanoTime()));
	}
}
