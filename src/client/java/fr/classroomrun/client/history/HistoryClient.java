package fr.classroomrun.client.history;

import com.mojang.blaze3d.platform.InputConstants;
import com.mojang.brigadier.CommandDispatcher;
import fr.classroomrun.net.HistoryPayload;
import fr.classroomrun.net.HistoryRequestPayload;
import net.fabricmc.fabric.api.client.command.v2.ClientCommands;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;

/** Entry points of the death history screen: key binding, {@code /crhistory}, request and answer. Main thread only. */
public final class HistoryClient {
	private static KeyMapping key;
	private static boolean openRequested;

	private HistoryClient() {
	}

	public static void register() {
		KeyMapping.Category category = KeyMapping.Category.register(Identifier.fromNamespaceAndPath("classroomrun", "main"));
		key = KeyMappingHelper.registerKeyMapping(
				new KeyMapping("key.classroomrun.history", InputConstants.KEY_H, category));
		ClientPlayNetworking.registerGlobalReceiver(HistoryPayload.TYPE, (payload, context) -> onAnswer(payload));
		ClientTickEvents.END_CLIENT_TICK.register(client -> {
			boolean pressed = false;
			while (key.consumeClick()) {
				pressed = true;
			}
			// the command flag is consumed here: opening a screen from inside the chat's own handler would be undone by it
			if ((pressed || openRequested) && client.player != null && (client.gui.screen() == null || openRequested)) {
				openRequested = false;
				client.gui.setScreen(new HistoryScreen());
			}
		});
	}

	public static void registerCommand(CommandDispatcher<FabricClientCommandSource> dispatcher) {
		dispatcher.register(ClientCommands.literal("crhistory").executes(c -> {
			openRequested = true;
			return 1;
		}));
	}

	/** Asks the server; it answers at most once per second per player and ignores the rest. */
	public static void request(int runNumber) {
		if (ClientPlayNetworking.canSend(HistoryRequestPayload.TYPE)) {
			ClientPlayNetworking.send(new HistoryRequestPayload(runNumber));
		}
	}

	private static void onAnswer(HistoryPayload payload) {
		if (Minecraft.getInstance().gui.screen() instanceof HistoryScreen screen) {
			screen.onAnswer(payload.runNumber(), payload.json());
		}
	}
}
