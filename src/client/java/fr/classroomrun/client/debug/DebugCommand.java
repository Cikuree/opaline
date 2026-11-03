package fr.classroomrun.client.debug;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import fr.classroomrun.client.ClientHandlers;
import fr.classroomrun.client.hud.ClientState;
import fr.classroomrun.net.DamagePayload;
import fr.classroomrun.net.DeathClipPayload;
import fr.classroomrun.net.HealPayload;
import fr.classroomrun.net.ReadyStatePayload;
import fr.classroomrun.net.RunStatePayload;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import net.fabricmc.fabric.api.client.command.v2.ClientCommands;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.network.chat.Component;

/** /crdebug: feeds the real payload handlers with fake payloads to test the HUD solo. */
public final class DebugCommand {
	private DebugCommand() {
	}

	public static void register(CommandDispatcher<FabricClientCommandSource> dispatcher) {
		dispatcher.register(ClientCommands.literal("crdebug")
				.then(ClientCommands.literal("state").executes(c -> state(c)))
				.then(ClientCommands.literal("damage")
						.then(ClientCommands.argument("hearts", FloatArgumentType.floatArg(0.1f))
								.executes(c -> damage(c, null))
								.then(ClientCommands.argument("name", StringArgumentType.word())
										.executes(c -> damage(c, StringArgumentType.getString(c, "name"))))))
				.then(ClientCommands.literal("heal")
						.then(ClientCommands.argument("hearts", FloatArgumentType.floatArg(0.1f))
								.executes(c -> heal(c))))
				.then(ClientCommands.literal("burst").executes(c -> burst(c)))
				.then(ClientCommands.literal("clip").executes(c -> clip(c)))
				.then(ClientCommands.literal("clear").executes(c -> {
					ClientState.reset();
					return reply(c, "etat efface");
				})));
	}

	private static int reply(CommandContext<FabricClientCommandSource> c, String msg) {
		c.getSource().sendFeedback(Component.literal(msg));
		return 1;
	}

	private static int state(CommandContext<FabricClientCommandSource> c) {
		// Run 3, 2 h 14 min, 7 deaths, splits 0-3 reached, split 4 in progress.
		ClientHandlers.onRunState(new RunStatePayload((2 * 60 + 14) * 60_000L, true, 7, 3, 4, 0b1111));
		return reply(c, "etat simule");
	}

	private static UUID self(CommandContext<FabricClientCommandSource> c) {
		return c.getSource().getPlayer().getUUID();
	}

	/** Tab-list player if the name matches, otherwise a stable fake UUID (default skin). */
	private static UUID resolve(CommandContext<FabricClientCommandSource> c, String name) {
		if (name == null) {
			return self(c);
		}
		ClientPacketListener conn = c.getSource().getClient().getConnection();
		PlayerInfo info = conn == null ? null : conn.getPlayerInfoIgnoreCase(name);
		return info != null ? info.getProfile().id() : fake(name);
	}

	private static UUID fake(String name) {
		return UUID.nameUUIDFromBytes(name.getBytes(StandardCharsets.UTF_8));
	}

	private static int damage(CommandContext<FabricClientCommandSource> c, String name) {
		ClientHandlers.onDamage(new DamagePayload(resolve(c, name), FloatArgumentType.getFloat(c, "hearts")));
		return 1;
	}

	private static int heal(CommandContext<FabricClientCommandSource> c) {
		ClientHandlers.onHeal(new HealPayload(self(c), FloatArgumentType.getFloat(c, "hearts")));
		return 1;
	}

	private static int burst(CommandContext<FabricClientCommandSource> c) {
		UUID a = fake("fake_a");
		UUID b = fake("fake_b");
		UUID d = fake("fake_c");
		ClientHandlers.onDamage(new DamagePayload(a, 1f));
		ClientHandlers.onDamage(new DamagePayload(b, 2f));
		ClientHandlers.onDamage(new DamagePayload(d, 0.5f));
		ClientHandlers.onDamage(new DamagePayload(a, 1f)); // quick re-hit: merges into "-2"
		return reply(c, "burst simule");
	}

	/**
	 * 5 s clip (10 frames/s) around the player: the player walks forward then crouches and is hurt on the last frames,
	 * a zombie closes in and swings, a skeleton stands still. Opens the playback screen like a real payload would.
	 */
	private static int clip(CommandContext<FabricClientCommandSource> c) {
		var player = c.getSource().getPlayer();
		int n = 50;
		float[] pp = new float[3 * n];
		byte[] pr = new byte[3 * n];
		byte[] pf = new byte[n];
		float[] zp = new float[3 * n];
		byte[] zr = new byte[3 * n];
		byte[] zf = new byte[n];
		for (int i = 0; i < n; i++) {
			pp[3 * i] = (i - n + 1) * 0.15f; // ends at the anchor
			pp[3 * i + 2] = 0f;
			pr[3 * i] = (byte) 64; // yaw 90 degrees
			pr[3 * i + 2] = (byte) 64;
			pf[i] = (byte) (DeathClipPayload.ON_GROUND | (i % 12 == 0 ? DeathClipPayload.SWING : 0)
					| (i >= n - 8 ? DeathClipPayload.CROUCHING : 0) | (i >= n - 3 ? DeathClipPayload.HURT : 0));
			zp[3 * i] = 6f - i * 0.12f;
			zp[3 * i + 2] = 2f;
			zr[3 * i] = (byte) 64;
			zr[3 * i + 2] = (byte) 64;
			zf[i] = (byte) (DeathClipPayload.ON_GROUND | (i >= n - 12 && i % 4 == 0 ? DeathClipPayload.SWING : 0));
		}
		float[] sp = new float[3 * n];
		byte[] sr = new byte[3 * n];
		byte[] sf = new byte[n];
		for (int i = 0; i < n; i++) {
			sp[3 * i] = -3f;
			sp[3 * i + 2] = -3f;
			sf[i] = (byte) DeathClipPayload.ON_GROUND;
		}
		var actors = List.of(
				new DeathClipPayload.Actor(0, "minecraft:player", Optional.of(player.getUUID()), player.getName().getString(),
						"minecraft:iron_sword", 0, pp, pr, pf),
				new DeathClipPayload.Actor(1, "minecraft:zombie", Optional.empty(), "", "minecraft:stone_axe", 0, zp, zr, zf),
				new DeathClipPayload.Actor(2, "minecraft:skeleton", Optional.empty(), "", "minecraft:bow", 0, sp, sr, sf));
		ClientHandlers.onDeathClip(new DeathClipPayload(player.getUUID(), player.level().dimension(), player.getX(),
				player.getY(), player.getZ(), 2, n, actors));
		fr.classroomrun.client.clip.ClipClient.markDebug();
		ClientHandlers.onReadyState(new ReadyStatePayload(1, 3, 45));
		return reply(c, "clip simule");
	}
}
