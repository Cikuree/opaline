package fr.classroomrun.client.clip;

import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElement;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.network.chat.Component;

/** Hint shown while a clip is available but its screen is closed, plus the ready counter. No allocation per frame. */
public final class ClipHud {
	private static final Component HINT = Component.literal("Clip de mort : appuie sur R pour le revoir");
	private static final int WHITE = 0xFFFFFFFF;
	private static final int YELLOW = 0xFFFFFF55;
	private static final int BOTTOM_MARGIN = 70;

	private ClipHud() {
	}

	public static HudElement element() {
		return ClipHud::extract;
	}

	private static void extract(GuiGraphicsExtractor g, DeltaTracker delta) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.gui.screen() instanceof DeathClipScreen) {
			return;
		}
		Font font = mc.font;
		int y = g.guiHeight() - BOTTOM_MARGIN;
		if (ClipClient.clip() != null) {
			g.centeredText(font, HINT, g.guiWidth() / 2, y, YELLOW);
			y += font.lineHeight + 2;
		}
		String ready = ClipClient.readyText();
		if (ready != null) {
			g.centeredText(font, ready, g.guiWidth() / 2, y, WHITE);
		}
	}
}
