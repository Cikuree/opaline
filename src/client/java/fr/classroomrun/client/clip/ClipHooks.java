package fr.classroomrun.client.clip;

import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;

/** Public entry points of the package-private {@link ClipStage} for the mixins. */
public final class ClipHooks {
	private ClipHooks() {
	}

	/** True when a clip is playing and {@code entity} is not one of its fakes. */
	public static boolean hides(Entity entity) {
		return ClipStage.active != null && !ClipStage.isFake(entity);
	}

	/** True when {@code living} is a clip fake whose walk animation was handled. */
	public static boolean walk(LivingEntity living) {
		return ClipStage.animateWalk(living);
	}
}
