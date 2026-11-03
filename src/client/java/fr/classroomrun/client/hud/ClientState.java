package fr.classroomrun.client.hud;

import fr.classroomrun.client.clip.ClipClient;
import fr.classroomrun.net.DeathPayload;

/** Single client-side holder of the mod state; read by the HUD, written by the network handlers. */
public final class ClientState {
	public static final ClientRunState RUN = new ClientRunState();
	public static final DamageFeed FEED = new DamageFeed();
	/** Last death received, kept for future use (no UI yet). */
	public static DeathPayload lastDeath;

	private ClientState() {
	}

	public static void reset() {
		RUN.reset();
		FEED.clear();
		lastDeath = null;
		ClipClient.clear();
	}
}
