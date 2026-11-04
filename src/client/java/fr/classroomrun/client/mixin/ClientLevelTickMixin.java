package fr.classroomrun.client.mixin;

import fr.classroomrun.client.clip.ClipHooks;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Why a mixin: projectile and TNT fakes must not run their own tick (arrows re-aim from their zero velocity, TNT burns its
 * fuse), and entity ticking has no Fabric event. Only clip fakes that are not living entities are skipped.
 */
@Mixin(ClientLevel.class)
public abstract class ClientLevelTickMixin {
	@Inject(method = "tickNonPassenger", at = @At("HEAD"), cancellable = true)
	private void classroomrun$freezeClipProjectiles(Entity entity, CallbackInfo ci) {
		if (ClipHooks.frozen(entity)) {
			ci.cancel();
		}
	}
}
