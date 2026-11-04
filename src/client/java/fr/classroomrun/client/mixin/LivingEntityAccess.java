package fr.classroomrun.client.mixin;

import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
import org.spongepowered.asm.mixin.gen.Invoker;

/**
 * Why a mixin: the client never runs {@code startUsingItem}'s flag update (server only), and the use progress of a clip fake
 * must follow the timeline (seek, pause), not the client ticks. Both members are protected and have no public setter.
 */
@Mixin(LivingEntity.class)
public interface LivingEntityAccess {
	@Invoker("setLivingEntityFlag")
	void classroomrun$setLivingEntityFlag(int flag, boolean value);

	@Accessor("useItemRemaining")
	void classroomrun$setUseItemRemaining(int ticks);
}
