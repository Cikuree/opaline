package fr.classroomrun.mixin;

import fr.classroomrun.sync.TeamSync;
import net.minecraft.server.level.ServerPlayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Why a mixin: same as {@link FoodDataMixin}, for the peaceful-difficulty regeneration of {@code tickRegeneration}. */
@Mixin(ServerPlayer.class)
public abstract class ServerPlayerMixin {
	@Inject(method = "tickRegeneration", at = @At("HEAD"), cancellable = true)
	private void classroomrun$naturalRegenStart(CallbackInfo ci) {
		if (!TeamSync.mayRunNaturalLogic((ServerPlayer) (Object) this)) {
			ci.cancel();
			return;
		}
		TeamSync.naturalRegeneration = true;
	}

	@Inject(method = "tickRegeneration", at = @At("RETURN"))
	private void classroomrun$naturalRegenEnd(CallbackInfo ci) {
		TeamSync.naturalRegeneration = false;
	}
}
