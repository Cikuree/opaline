package fr.classroomrun.client.clip;

import fr.classroomrun.net.DeathClipPayload;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.Block;

/**
 * Shows the terrain of the clip: sets each changed position to the state it had at the current frame with client-only writes
 * (flags UPDATE_CLIENTS | UPDATE_KNOWN_SHAPE: re-render and light, no neighbour or shape updates; nothing is sent, and the
 * prediction handler only reacts while the player is predicting an action). Only positions whose displayed state changes are
 * touched; {@link #restore} puts back the states the client really had. Positions in unloaded chunks are skipped. Main thread.
 */
final class ClipTerrain {
	private static final int FLAGS = Block.UPDATE_CLIENTS | Block.UPDATE_KNOWN_SHAPE;

	private final ClientLevel level;
	private final ClipBlocks blocks;
	private final BlockPos[] pos;
	/** Displayed replay state id per position, -1 while the real state is displayed. */
	private final int[] shown;
	/** Real state id, captured right before the first write. */
	private final int[] orig;
	private int lastFrame = -1;

	ClipTerrain(ClientLevel level, DeathClipPayload clip) {
		this.level = level;
		this.blocks = new ClipBlocks(clip.blocks());
		int n = this.blocks.positions.length;
		BlockPos anchor = BlockPos.containing(clip.anchorX(), clip.anchorY(), clip.anchorZ());
		this.pos = new BlockPos[n];
		this.shown = new int[n];
		this.orig = new int[n];
		for (int i = 0; i < n; i++) {
			ClipBlocks.Pos p = this.blocks.positions[i];
			this.pos[i] = anchor.offset(p.dx(), p.dy(), p.dz());
			this.shown[i] = -1;
		}
	}

	void update(int frame) {
		if (frame == this.lastFrame) {
			return;
		}
		this.lastFrame = frame;
		for (int i = 0; i < this.pos.length; i++) {
			int target = this.blocks.positions[i].stateAt(frame);
			if (!this.level.hasChunkAt(this.pos[i])) {
				continue;
			}
			if (this.shown[i] == -1) {
				int real = Block.getId(this.level.getBlockState(this.pos[i]));
				if (real != target && put(i, target)) {
					this.orig[i] = real;
					this.shown[i] = target;
				}
			} else if (this.shown[i] != target && put(i, target)) {
				this.shown[i] = target == this.orig[i] ? -1 : target;
			}
		}
	}

	/** Gives back the real states, except where something else changed the block meanwhile. */
	void restore() {
		for (int i = 0; i < this.pos.length; i++) {
			if (this.shown[i] != -1 && this.level.hasChunkAt(this.pos[i])
					&& Block.getId(this.level.getBlockState(this.pos[i])) == this.shown[i]) {
				put(i, this.orig[i]);
			}
			this.shown[i] = -1;
		}
	}

	private boolean put(int i, int stateId) {
		return this.level.setBlock(this.pos[i], Block.stateById(stateId), FLAGS);
	}
}
