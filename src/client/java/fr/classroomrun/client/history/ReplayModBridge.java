package fr.classroomrun.client.history;

import com.replaymod.replay.ReplayHandler;
import com.replaymod.replay.ReplayModReplay;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;

import java.io.IOException;
import java.nio.file.Path;

/**
 * Every ReplayMod reference of the mod. Loaded only when {@code replaymod} is present ({@link HistoryClient#replayModLoaded()}).
 * Call on the client main thread, once the client is disconnected from the server: neither {@code startReplay} nor
 * {@code ReplayHandler} disconnects.
 */
final class ReplayModBridge {
	/** Ticks to wait after startReplay before jumping: the replay sender needs a few ticks to be ready. */
	private static final int JUMP_DELAY_TICKS = 10;
	private static final int GIVE_UP_TICKS = 400;

	private static boolean registered;
	private static long pendingOffsetMs = -1;
	private static int ticks;

	private ReplayModBridge() {
	}

	/** Opens the file in ReplayMod's viewer and jumps to {@code offsetMs} a few ticks later. */
	static void open(Path file, long offsetMs) throws IOException {
		ReplayModReplay replay = ReplayModReplay.instance;
		if (replay == null) {
			throw new IOException("ReplayMod n'est pas initialise");
		}
		replay.startReplay(file.toFile());
		pendingOffsetMs = Math.min(Integer.MAX_VALUE, Math.max(0, offsetMs));
		ticks = 0;
		if (!registered) {
			registered = true;
			ClientTickEvents.END_CLIENT_TICK.register(client -> tick());
		}
	}

	private static void tick() {
		if (pendingOffsetMs < 0) {
			return;
		}
		ticks++;
		ReplayHandler handler = ReplayModReplay.instance.getReplayHandler();
		if (handler != null && ticks >= JUMP_DELAY_TICKS) {
			handler.doJump((int) pendingOffsetMs, false);
			pendingOffsetMs = -1;
		} else if (ticks > GIVE_UP_TICKS) {
			pendingOffsetMs = -1;
		}
	}
}
