package fr.classroomrun.journal;

import net.minecraft.server.level.ServerPlayer;
import org.jspecify.annotations.Nullable;

/**
 * What the journal needs from a replay recorder. The only implementation, {@link ServerReplayHook}, touches ServerReplay
 * classes and is instantiated only when the {@code server-replay} mod is loaded; this interface has no such reference.
 */
public interface ReplayHook {
	/** A position in a recording file: path relative to the game's {@code recordings} directory (opaque, '/' separated). */
	record Ref(String path, long offsetMs) {
	}

	/** Starts (once) the recording of this player for this run. */
	void start(ServerPlayer player, int run);

	/** Stops and saves this player's recording (asynchronous). */
	void stop(ServerPlayer player);

	/** Stops and saves every recording started by this hook (asynchronous). */
	void stopAll();

	/** Waits for the saves started by {@link #stop} and {@link #stopAll}. */
	void awaitSaved(long timeoutMs);

	/** Adds a marker to every recording. @return where this player's death is in this player's own file, if recorded */
	@Nullable Ref marker(String label, @Nullable ServerPlayer victim);
}
