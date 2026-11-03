package fr.classroomrun.client;

import fr.classroomrun.client.clip.ClipClient;
import fr.classroomrun.client.hud.ClientState;
import fr.classroomrun.net.DamagePayload;
import fr.classroomrun.net.DeathClipPayload;
import fr.classroomrun.net.DeathPayload;
import fr.classroomrun.net.HealPayload;
import fr.classroomrun.net.ReadyStatePayload;
import fr.classroomrun.net.RunStatePayload;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.util.Util;

/** Payload handlers, shared by the real network receivers and the /crdebug command. Run on the client main thread. */
public final class ClientHandlers {
	private ClientHandlers() {
	}

	public static void register() {
		// Handlers run on the main client thread (packet processor), see docs/api/client-and-network.md 1.5.
		ClientPlayNetworking.registerGlobalReceiver(RunStatePayload.TYPE, (payload, context) -> onRunState(payload));
		ClientPlayNetworking.registerGlobalReceiver(DamagePayload.TYPE, (payload, context) -> onDamage(payload));
		ClientPlayNetworking.registerGlobalReceiver(HealPayload.TYPE, (payload, context) -> onHeal(payload));
		ClientPlayNetworking.registerGlobalReceiver(DeathPayload.TYPE, (payload, context) -> onDeath(payload));
		ClientPlayNetworking.registerGlobalReceiver(DeathClipPayload.TYPE, (payload, context) -> onDeathClip(payload));
		ClientPlayNetworking.registerGlobalReceiver(ReadyStatePayload.TYPE, (payload, context) -> onReadyState(payload));
		ClientPlayConnectionEvents.DISCONNECT.register((listener, client) -> ClientState.reset());
	}

	public static void onRunState(RunStatePayload p) {
		ClientState.RUN.update(p.totalTimeMs(), p.running(), p.deaths(), p.runNumber(), p.currentSplit(),
				p.reachedSplits(), Util.getMillis());
		ClipClient.onRunState(p);
	}

	public static void onDamage(DamagePayload p) {
		ClientState.FEED.add(p.player(), p.hearts(), false, Util.getMillis());
	}

	public static void onHeal(HealPayload p) {
		ClientState.FEED.add(p.player(), p.hearts(), true, Util.getMillis());
	}

	public static void onDeath(DeathPayload p) {
		ClientState.lastDeath = p;
	}

	public static void onDeathClip(DeathClipPayload p) {
		ClipClient.onClip(p, ClientState.RUN.runNumber());
	}

	public static void onReadyState(ReadyStatePayload p) {
		ClipClient.onReadyState(p);
	}
}
