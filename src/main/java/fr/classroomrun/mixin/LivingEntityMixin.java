package fr.classroomrun.mixin;

import fr.classroomrun.sync.TeamSync;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Why a mixin: Fabric API 26.3 has no heal event. Reports the health really gained by a player so the shared heal
 * can be announced with a {@code HealPayload}; the propagation itself is done per tick and does not go through here.
 */
@Mixin(LivingEntity.class)
public abstract class LivingEntityMixin {
	@Inject(method = "heal", at = @At("HEAD"))
	private void classroomrun$reportHeal(float amount, CallbackInfo ci) {
		if ((Object) this instanceof ServerPlayer player) {
			float health = player.getHealth();
			if (health > 0.0F) {
				TeamSync.onHeal(player, Math.min(health + amount, player.getMaxHealth()) - health);
			}
		}
	}
}
