package fr.classroomrun.client.clip;

import com.mojang.blaze3d.platform.InputConstants;
import fr.classroomrun.client.history.HistoryClient;
import fr.classroomrun.net.ClipReadyPayload;
import fr.classroomrun.net.DeathClipPayload;
import fr.classroomrun.net.ReadyStatePayload;
import fr.classroomrun.net.RunStatePayload;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;

/** Holder of the current death clip and the ready counter; entry points of the screen. Main thread only. */
public final class ClipClient {
	private static KeyMapping key;
	private static DeathClipPayload clip;
	private static int clipRun;
	private static boolean pendingOpen;
	private static String readyText;
	private static boolean debugClip;

	private ClipClient() {
	}

	/** Must run after {@link HistoryClient#register()}, which registers the key category. */
	public static void register() {
		key = KeyMappingHelper.registerKeyMapping(
				new KeyMapping("key.classroomrun.clip", InputConstants.KEY_R, HistoryClient.category));
		ClientTickEvents.END_CLIENT_TICK.register(ClipClient::tick);
	}

	public static DeathClipPayload clip() {
		return clip;
	}

	public static String readyText() {
		return readyText;
	}

	/** Stores the clip and opens the screen on the next tick (never from inside a chat or packet handler). */
	public static void onClip(DeathClipPayload payload, int runNumber) {
		clip = payload;
		clipRun = runNumber;
		debugClip = false;
		pendingOpen = true;
	}

	/** The debug clip is local only: « Prêt » must not notify the server. Call right after {@link #onClip}. */
	public static void markDebug() {
		debugClip = true;
	}

	public static void onReadyState(ReadyStatePayload p) {
		if (p.total() == 0 && p.secondsLeft() == 0) { // explicit end signal of the server
			clear();
			return;
		}
		readyText = "Prêts : " + p.ready() + "/" + p.total() + " · nouvelle run dans " + p.secondsLeft() + " s";
	}

	/** A running timer or a new run number means the death is over. */
	public static void onRunState(RunStatePayload p) {
		if (p.running() || p.runNumber() != clipRun) {
			clear();
		}
	}

	/** Drops everything (new run, disconnect); an open screen notices it on its next tick and closes itself. */
	public static void clear() {
		clip = null;
		readyText = null;
		pendingOpen = false;
	}

	/** The « Prêt » button: deletes the clip, tells the server, closes the screen. Does nothing without a clip. */
	static void ready() {
		if (clip == null) {
			return;
		}
		clip = null;
		pendingOpen = false;
		if (!debugClip && ClientPlayNetworking.canSend(ClipReadyPayload.TYPE)) {
			ClientPlayNetworking.send(ClipReadyPayload.INSTANCE);
		}
		Minecraft.getInstance().gui.setScreen(null);
	}

	private static void tick(Minecraft client) {
		boolean pressed = false;
		while (key.consumeClick()) {
			pressed = true;
		}
		if (clip == null || client.player == null || client.level == null) {
			return;
		}
		Screen current = client.gui.screen();
		if (pendingOpen || pressed && current == null) {
			pendingOpen = false;
			// auto-open only when the clip belongs to this dimension; otherwise the hint stays and R opens the UI-only screen
			if (current instanceof DeathClipScreen) {
				return;
			}
			if (pressed || (client.player.isSpectator() || debugClip) && client.level.dimension().equals(clip.dimension())) {
				client.gui.setScreen(new DeathClipScreen(clip));
			}
		}
	}
}
