package fr.classroomrun.client.history;

import com.replaymod.replay.ReplayHandler;
import com.replaymod.replay.ReplayModReplay;

import java.io.IOException;
import java.nio.file.Path;

/**
 * Every ReplayMod reference of the mod. Loaded only when {@code replaymod} is present ({@link HistoryClient#replayModLoaded()}).
 * Call on the client main thread.
 */
final class ReplayModBridge {
	private ReplayModBridge() {
	}

	/** Opens the file in ReplayMod's viewer and jumps to {@code offsetMs}. */
	static void open(Path file, long offsetMs) throws IOException {
		ReplayModReplay replay = ReplayModReplay.instance;
		if (replay == null) {
			throw new IOException("ReplayMod n'est pas initialise");
		}
		replay.startReplay(file.toFile());
		ReplayHandler handler = replay.getReplayHandler();
		if (handler != null) {
			handler.doJump((int) Math.min(Integer.MAX_VALUE, Math.max(0, offsetMs)), false);
		}
	}
}
