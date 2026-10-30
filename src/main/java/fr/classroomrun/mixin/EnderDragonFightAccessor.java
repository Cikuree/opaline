package fr.classroomrun.mixin;

import net.minecraft.server.level.ServerBossEvent;
import net.minecraft.world.level.dimension.end.EnderDragonFight;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Why a mixin: the dragon boss bar is a private field and no API detaches its players when a level is deleted. */
@Mixin(EnderDragonFight.class)
public interface EnderDragonFightAccessor {
	@Accessor("dragonEvent")
	ServerBossEvent classroomrun$getDragonEvent();
}
