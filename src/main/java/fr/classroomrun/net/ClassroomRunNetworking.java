package fr.classroomrun.net;

import fr.classroomrun.ClassroomRun;
import net.fabricmc.fabric.api.networking.v1.PayloadTypeRegistry;
import net.minecraft.resources.Identifier;

/** Registers the server → client payload types. Called once from the common entrypoint. */
public final class ClassroomRunNetworking {
	private ClassroomRunNetworking() {
	}

	static Identifier id(String path) {
		return Identifier.fromNamespaceAndPath(ClassroomRun.MOD_ID, path);
	}

	public static void registerPayloads() {
		PayloadTypeRegistry.clientboundPlay().register(RunStatePayload.TYPE, RunStatePayload.CODEC);
		PayloadTypeRegistry.clientboundPlay().register(DamagePayload.TYPE, DamagePayload.CODEC);
		PayloadTypeRegistry.clientboundPlay().register(HealPayload.TYPE, HealPayload.CODEC);
		PayloadTypeRegistry.clientboundPlay().register(DeathPayload.TYPE, DeathPayload.CODEC);
	}
}
