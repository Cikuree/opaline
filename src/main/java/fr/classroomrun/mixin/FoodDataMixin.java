package fr.classroomrun.mixin;

import fr.classroomrun.sync.TeamSync;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.food.FoodData;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Why a mixin: natural regeneration calls {@code LivingEntity.heal} from {@code FoodData.tick}; flagging that call
 * is the only way to tell it from a potion or an apple (the contract says natural regeneration is not announced).
 */
@Mixin(FoodData.class)
public abstract class FoodDataMixin {
	@Inject(method = "tick", at = @At("HEAD"))
	private void classroomrun$naturalRegenStart(ServerPlayer player, CallbackInfo ci) {
		TeamSync.naturalRegeneration = true;
	}

	@Inject(method = "tick", at = @At("RETURN"))
	private void classroomrun$naturalRegenEnd(ServerPlayer player, CallbackInfo ci) {
		TeamSync.naturalRegeneration = false;
	}
}
