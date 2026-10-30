package fr.classroomrun.mixin;

import fr.classroomrun.game.RunManager;
import fr.classroomrun.game.RunWorlds;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Relative;
import net.minecraft.world.level.block.EndPortalBlock;
import net.minecraft.world.level.levelgen.feature.EndPlatformFeature;
import net.minecraft.world.level.portal.TeleportTransition;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.Set;

/**
 * Why a mixin: {@code EndPortalBlock.getPortalDestination} hard-codes {@code Level.END} and the world respawn
 * dimension; no Fabric event lets us choose the destination. From a run Overworld the portal leads to the run End
 * (same arrival platform as vanilla); from the run End it leads back to the run classroom. Other levels keep the
 * vanilla behaviour. The dragon fight of the run End exists because its dimension type is the vanilla End one
 * ({@code hasEnderDragonFight()}, checked in the {@code ServerLevel} constructor).
 */
@Mixin(EndPortalBlock.class)
public abstract class EndPortalBlockMixin {
	@Inject(method = "getPortalDestination", at = @At("HEAD"), cancellable = true)
	private void classroomrun$runTrioDestination(ServerLevel currentLevel, Entity entity, BlockPos portalEntryPos,
			CallbackInfoReturnable<TeleportTransition> cir) {
		RunManager run = RunManager.get();
		RunWorlds worlds = run == null ? null : run.worlds();
		if (worlds == null || !worlds.contains(currentLevel)) {
			return;
		}
		if (currentLevel == worlds.end()) {
			// only players go back to the classroom; other entities are not teleported
			cir.setReturnValue(entity instanceof ServerPlayer ? run.classroomTransition() : null);
			return;
		}
		ServerLevel end = worlds.end();
		Vec3 spawnPos = Vec3.atBottomCenterOf(ServerLevel.END_SPAWN_POINT);
		EndPlatformFeature.createEndPlatform(end, BlockPos.containing(spawnPos).below(), true);
		if (entity instanceof ServerPlayer) {
			spawnPos = spawnPos.subtract(0.0, 1.0, 0.0);
		}
		cir.setReturnValue(new TeleportTransition(end, spawnPos, Vec3.ZERO, Direction.WEST.toYRot(), 0.0F,
				Relative.union(Relative.DELTA, Set.of(Relative.X_ROT)),
				TeleportTransition.PLAY_PORTAL_SOUND.then(TeleportTransition.PLACE_PORTAL_TICKET)));
	}
}
