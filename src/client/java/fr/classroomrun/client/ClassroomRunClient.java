package fr.classroomrun.client;

import fr.classroomrun.client.clip.ClipClient;
import fr.classroomrun.client.clip.ClipHud;
import fr.classroomrun.client.debug.DebugCommand;
import fr.classroomrun.client.history.HistoryClient;
import fr.classroomrun.client.hud.HudRenderer;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;
import net.minecraft.resources.Identifier;

public class ClassroomRunClient implements ClientModInitializer {
	@Override
	public void onInitializeClient() {
		ClientHandlers.register();
		HistoryClient.register();
		ClipClient.register();
		HudElementRegistry.attachElementAfter(VanillaHudElements.HEALTH_BAR,
				Identifier.fromNamespaceAndPath("classroomrun", "damage_feed"), HudRenderer.feedElement());
		HudElementRegistry.attachElementAfter(VanillaHudElements.MISC_OVERLAYS,
				Identifier.fromNamespaceAndPath("classroomrun", "splits"), HudRenderer.splitsElement());
		HudElementRegistry.attachElementAfter(VanillaHudElements.MISC_OVERLAYS,
				Identifier.fromNamespaceAndPath("classroomrun", "clip_hint"), ClipHud.element());
		ClientCommandRegistrationCallback.EVENT.register((dispatcher, access) -> {
			DebugCommand.register(dispatcher);
			HistoryClient.registerCommand(dispatcher);
		});
	}
}
