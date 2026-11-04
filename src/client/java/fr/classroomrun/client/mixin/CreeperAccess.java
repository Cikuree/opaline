package fr.classroomrun.client.mixin;

import net.minecraft.world.entity.monster.Creeper;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Why a mixin: {@code swell} / {@code oldSwell} (what the renderer reads through {@code getSwelling}) are private. */
@Mixin(Creeper.class)
public interface CreeperAccess {
	@Accessor("swell")
	void classroomrun$setSwell(int swell);

	@Accessor("oldSwell")
	void classroomrun$setOldSwell(int swell);
}
