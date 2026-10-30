package fr.classroomrun.mixin;

import fr.classroomrun.sync.TeamSync;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Why a mixin: food exhaustion is private to each {@code FoodData}. The exhaustion caused by a non-carrier member is
 * added to the carrier instead, so the team's hunger equals that of one player doing all the actions.
 */
@Mixin(Player.class)
public abstract class PlayerMixin {
	@Inject(method = "causeFoodExhaustion", at = @At("HEAD"), cancellable = true)
	private void classroomrun$shareExhaustion(float amount, CallbackInfo ci) {
		if ((Object) this instanceof ServerPlayer player) {
			if (TeamSync.shareExhaustion(player, amount)) {
				ci.cancel();
			}
		}
	}
}
