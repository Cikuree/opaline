package fr.classroomrun.client.debug;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import fr.classroomrun.client.ClientHandlers;
import fr.classroomrun.client.hud.ClientState;
import fr.classroomrun.clip.ClipExtra;
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

	/** Per-frame arrays of one debug actor. */
	private static final class Track {
		final float[] pos;
		final byte[] rot, flags, extra;

		Track(int frames) {
			this.pos = new float[3 * frames];
			this.rot = new byte[3 * frames];
			this.flags = new byte[frames];
			this.extra = new byte[frames];
		}

		/** @param yaw yaw in 1/256 turns (0 faces +z, 64 faces -x), also used as head yaw */
		void at(int i, float x, float y, float z, int yaw, int flags, int extra) {
			this.pos[3 * i] = x;
			this.pos[3 * i + 1] = y;
			this.pos[3 * i + 2] = z;
			this.rot[3 * i] = (byte) yaw;
			this.rot[3 * i + 2] = (byte) yaw;
			this.flags[i] = (byte) flags;
			this.extra[i] = (byte) extra;
		}

		DeathClipPayload.Actor actor(int id, String type, Optional<UUID> profile, String name, String main, int first) {
			return new DeathClipPayload.Actor(id, type, profile, name, main, "", first, this.pos, this.rot, this.flags,
					this.extra);
		}
	}

	/**
	 * 8 s clip (10 frames/s) around the player. The player (glowing, name tag) walks along +x, is hit by an arrow on the last
	 * frames and plays the death animation at the end. A skeleton 8 blocks north draws its bow (frames 40-65) and shoots the
	 * arrow (frames 66-78). A creeper swells from frame 30 and explodes at frame 60 (particles, sound, crater). A zombie
	 * closes in with its arms up.
	 */
	private static int clip(CommandContext<FabricClientCommandSource> c) {
		var player = c.getSource().getPlayer();
		int n = 80;
		int ground = DeathClipPayload.ON_GROUND;
		Track p = new Track(n);
		Track z = new Track(n);
		Track sk = new Track(n);
		for (int i = 0; i < n; i++) {
			p.at(i, (i - n + 1) * 0.1f, 0f, 0f, -64,
					ground | (i % 12 == 0 ? DeathClipPayload.SWING : 0) | (i >= n - 3 ? DeathClipPayload.HURT : 0), 0);
			z.at(i, 6f - i * 0.06f, 0f, 2f, 64,
					ground | DeathClipPayload.AGGRESSIVE | (i >= n - 12 && i % 4 == 0 ? DeathClipPayload.SWING : 0), 0);
			sk.at(i, 0f, 0f, -8f, 0,
					ground | DeathClipPayload.AGGRESSIVE | (i >= 40 && i < 66 ? DeathClipPayload.USING_ITEM : 0), 0);
		}
		Track cr = new Track(60);
		for (int i = 0; i < 60; i++) {
			cr.at(i, 4f, 0f, 4f, 96, ground, i < 30 ? 0 : ClipExtra.swellLevel((i - 30) / 28f));
		}
		Track arrow = new Track(13);
		for (int j = 0; j < 13; j++) {
			arrow.at(j, 0f, 1.4f - j * 0.02f, -7f + j * 0.55f, 0, 0, 0);
		}
		var actors = List.of(
				p.actor(0, "minecraft:player", Optional.of(player.getUUID()), player.getName().getString(), "minecraft:iron_sword", 0),
				z.actor(1, "minecraft:zombie", Optional.empty(), "", "minecraft:stone_axe", 0),
				sk.actor(2, "minecraft:skeleton", Optional.empty(), "", "minecraft:bow", 0),
				cr.actor(3, "minecraft:creeper", Optional.empty(), "", "", 0),
				arrow.actor(4, "minecraft:arrow", Optional.empty(), "", "", 66));
		int dirt = net.minecraft.world.level.block.Block.getId(net.minecraft.world.level.block.Blocks.DIRT.defaultBlockState());
		java.util.ArrayList<DeathClipPayload.BlockChange> crater = new java.util.ArrayList<>();
		for (int dx = 3; dx <= 5; dx++) {
			for (int dz = 3; dz <= 5; dz++) {
				crater.add(new DeathClipPayload.BlockChange(60, dx, -1, dz, dirt, 0));
			}
		}
		crater.add(new DeathClipPayload.BlockChange(60, 4, -2, 4, dirt, 0));
		ClientHandlers.onDeathClip(new DeathClipPayload(player.getUUID(), player.level().dimension(), player.getX(),
				player.getY(), player.getZ(), 2, n, actors, crater, List.of(new DeathClipPayload.Explosion(60, 4f, 0.5f, 4f, 3f))));
		fr.classroomrun.client.clip.ClipClient.markDebug();
		ClientHandlers.onReadyState(new ReadyStatePayload(1, 3, 45));
		return reply(c, "clip simule");
	}
}
