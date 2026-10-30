package fr.classroomrun.mixin;

import fr.classroomrun.game.RunManager;
import fr.classroomrun.game.RunWorlds;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.NetherPortalBlock;
import net.minecraft.world.level.border.WorldBorder;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.portal.TeleportTransition;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Why a mixin: {@code NetherPortalBlock.getPortalDestination} hard-codes {@code Level.NETHER} / {@code Level.OVERWORLD}
 * and looks the target up with {@code server.getLevel(...)}; Fantasy levels have their own keys, and no Fabric event
 * lets us choose the portal destination. For a portal inside a run level, the destination is computed in the run trio
 * (overworld run to nether run and back) with the vanilla algorithm; any other level keeps the vanilla behaviour.
 */
@Mixin(NetherPortalBlock.class)
public abstract class NetherPortalBlockMixin {
	@Shadow
	protected abstract TeleportTransition getExitPortal(ServerLevel newLevel, Entity entity, BlockPos portalEntryPos,
			BlockPos approximateExitPos, boolean toNether, WorldBorder worldBorder);

	@Inject(method = "getPortalDestination", at = @At("HEAD"), cancellable = true)
	private void classroomrun$runTrioDestination(ServerLevel currentLevel, Entity entity, BlockPos portalEntryPos,
			CallbackInfoReturnable<TeleportTransition> cir) {
		RunManager run = RunManager.get();
		RunWorlds worlds = run == null ? null : run.worlds();
		if (worlds == null || !worlds.contains(currentLevel)) {
			return;
		}
		ServerLevel target = currentLevel == worlds.nether() ? worlds.overworld() : worlds.nether();
		boolean toNether = target == worlds.nether();
		WorldBorder border = target.getWorldBorder();
		double scale = DimensionType.getTeleportationScale(currentLevel.dimensionType(), target.dimensionType());
		BlockPos approximateExitPos = border.clampToBounds(entity.getX() * scale, entity.getY(), entity.getZ() * scale);
		cir.setReturnValue(this.getExitPortal(target, entity, portalEntryPos, approximateExitPos, toNether, border));
	}
}
