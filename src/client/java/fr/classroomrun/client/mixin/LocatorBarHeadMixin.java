package fr.classroomrun.client.mixin;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.PlayerFaceExtractor;
import net.minecraft.client.gui.contextualbar.LocatorBar;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.resources.Identifier;
import net.minecraft.world.waypoints.TrackedWaypoint;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Locator bar: a player's waypoint shows that player's skin face (framed in the waypoint colour) instead of the dot.
 * Non-player waypoints and players missing from the tab list keep the vanilla dot.
 */
@Mixin(LocatorBar.class)
abstract class LocatorBarHeadMixin {
	/** The per-waypoint lambda of {@code extractRenderState} (26.3: {@code lambda$extractRenderState$1}, last arg the waypoint); require 0: a renamed lambda in a later version only loses the heads, never crashes. */
	@WrapOperation(method = "lambda$extractRenderState$1", require = 0, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIIII)V"))
	private void classroomrun$playerHead(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h,
			int color, Operation<Void> original, @Local(argsOnly = true) TrackedWaypoint waypoint) {
		ClientPacketListener conn = Minecraft.getInstance().getConnection();
		PlayerInfo info = conn == null ? null : waypoint.id().left().map(conn::getPlayerInfo).orElse(null);
		if (info == null) {
			original.call(g, pipeline, sprite, x, y, w, h, color);
			return;
		}
		g.fill(x, y, x + w, y + h, color | 0xFF000000); // 1px frame in the player's waypoint colour
		PlayerFaceExtractor.extractRenderState(g, info.getSkin(), x + 1, y + 1, w - 2);
	}
}
