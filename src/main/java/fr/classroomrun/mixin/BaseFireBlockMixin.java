package fr.classroomrun.mixin;

import fr.classroomrun.game.RunManager;
import fr.classroomrun.game.RunWorlds;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.BaseFireBlock;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Why a mixin: {@code BaseFireBlock.inPortalDimension} (private static) compares the dimension key with
 * {@code Level.OVERWORLD} / {@code Level.NETHER}, so a Nether portal could never be lit in a Fantasy level.
 * No event covers this; the run Overworld and run Nether are accepted too.
 */
@Mixin(BaseFireBlock.class)
public abstract class BaseFireBlockMixin {
	@Inject(method = "inPortalDimension", at = @At("HEAD"), cancellable = true)
	private static void classroomrun$runPortalDimension(Level level, CallbackInfoReturnable<Boolean> cir) {
		RunManager run = RunManager.get();
		RunWorlds worlds = run == null ? null : run.worlds();
		if (worlds != null && (level == worlds.overworld() || level == worlds.nether())) {
			cir.setReturnValue(true);
		}
	}
}
