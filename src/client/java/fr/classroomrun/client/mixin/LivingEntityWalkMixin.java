package fr.classroomrun.client.mixin;

import fr.classroomrun.client.clip.ClipHooks;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Walk animation of the clip fakes, which the vanilla position-delta computation cannot see. */
@Mixin(LivingEntity.class)
public abstract class LivingEntityWalkMixin {
	@Inject(method = "calculateEntityAnimation", at = @At("HEAD"), cancellable = true)
	private void classroomrun$clipWalk(boolean useY, CallbackInfo ci) {
		if (ClipHooks.walk((LivingEntity) (Object) this)) {
			ci.cancel();
		}
	}
}
