package fr.classroomrun.client.debug;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.FloatArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import fr.classroomrun.client.ClientHandlers;
import fr.classroomrun.client.hud.ClientState;
import fr.classroomrun.net.DamagePayload;
import fr.classroomrun.net.HealPayload;
import fr.classroomrun.net.RunStatePayload;
import java.nio.charset.StandardCharsets;
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
}
