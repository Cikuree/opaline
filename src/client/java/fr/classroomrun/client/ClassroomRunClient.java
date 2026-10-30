package fr.classroomrun.client;

import fr.classroomrun.client.debug.DebugCommand;
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
		HudElementRegistry.attachElementAfter(VanillaHudElements.HEALTH_BAR,
				Identifier.fromNamespaceAndPath("classroomrun", "damage_feed"), HudRenderer.feedElement());
		HudElementRegistry.attachElementAfter(VanillaHudElements.BOSS_BAR,
				Identifier.fromNamespaceAndPath("classroomrun", "splits"), HudRenderer.splitsElement());
		ClientCommandRegistrationCallback.EVENT.register((dispatcher, access) -> DebugCommand.register(dispatcher));
	}
}
