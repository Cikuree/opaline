package fr.classroomrun.mixin;

import fr.classroomrun.game.RunManager;
import net.minecraft.core.BlockPos;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Why a mixin: the death clip replays terrain changes, and no Fabric event covers every block change (explosions, fire,
 * pistons, endermen, falling blocks, players). {@code Level.setBlock} is the single choke point they all go through.
 * Redirecting its {@code LevelChunk.setBlockState} call gives the old state as the return value (null = nothing changed),
 * so there is no HEAD/RETURN state to carry across the recursive neighbour updates.
 */
@Mixin(Level.class)
public abstract class LevelBlockRecordMixin {
	@Redirect(method = "setBlock(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;II)Z",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/chunk/LevelChunk;setBlockState(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;I)Lnet/minecraft/world/level/block/state/BlockState;"))
	private BlockState classroomrun$recordBlock(LevelChunk chunk, BlockPos pos, BlockState state, int flags) {
		BlockState old = chunk.setBlockState(pos, state, flags);
		if (old != null && (Object) this instanceof ServerLevel level) {
			RunManager run = RunManager.get();
			if (run != null) {
				run.clips().onBlockChange(level, pos, old, state);
			}
		}
		return old;
	}
}
