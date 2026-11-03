package fr.classroomrun.client.mixin;

import fr.classroomrun.client.clip.ClipHooks;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** While a death clip plays, only its fake entities are rendered. */
@Mixin(LevelExtractor.class)
public abstract class LevelExtractorMixin {
	@Inject(method = "isEntityVisible", at = @At("HEAD"), cancellable = true)
	private void classroomrun$onlyClipEntities(Entity entity, Frustum frustum, double camX, double camY, double camZ,
			float partialTicks, long chunkFadeDuration, CallbackInfoReturnable<Boolean> cir) {
		if (ClipHooks.hides(entity)) {
			cir.setReturnValue(false);
		}
	}
}
