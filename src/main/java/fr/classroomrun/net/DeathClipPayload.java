package fr.classroomrun.net;

import io.netty.buffer.ByteBuf;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.core.UUIDUtil;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.codec.ByteBufCodecs;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.Level;

/**
 * Clip of the last minute before the collective death, sent to every client right after {@link DeathPayload}.
 * Positions are relative to {@code anchor} (the victim's position at death). One frame every {@code sampleTicks} ticks;
 * frame {@code frameCount - 1} is the death.
 *
 * @param victim      UUID of the player who took the fatal hit (an actor of the clip)
 * @param dimension   dimension the clip was recorded in (the client only plays it when it is in that dimension)
 * @param anchorX     anchor, world coordinates
 * @param sampleTicks game ticks between two frames (2 = 10 frames per second)
 * @param frameCount  number of frames of the clip
 * @param actors      recorded entities, the victim included
 * @param blocks      block changes around the anchor, ordered by time (see {@link BlockChange})
 * @param explosions  explosions near the anchor, ordered by time (see {@link Explosion})
 */
public record DeathClipPayload(UUID victim, ResourceKey<Level> dimension, double anchorX, double anchorY, double anchorZ,
		int sampleTicks, int frameCount, List<Actor> actors, List<BlockChange> blocks, List<Explosion> explosions)
		implements CustomPacketPayload {
	public static final Type<DeathClipPayload> TYPE = new Type<>(ClassroomRunNetworking.id("death_clip"));

	/** Per-frame flag bits of {@link Actor#flags()}. */
	public static final int ON_GROUND = 1, CROUCHING = 1 << 1, SPRINTING = 1 << 2, SWING = 1 << 3, HURT = 1 << 4,
			FALL_FLYING = 1 << 5, USING_ITEM = 1 << 6, AGGRESSIVE = 1 << 7;
	/**
	 * Hard caps so the payload stays below the 1 MiB custom payload limit. Worst case, every actor present for every frame:
	 * 64 actors * 600 frames * 17 bytes (3 floats + 3 angle bytes + flags + extra) = 653 KB, + 4096 block changes * 11 bytes
	 * (frame 2, dx/dy/dz 3, two 3-byte state ids) = 45 KB, + 64 explosions * 18 bytes = 1 KB, + strings (64 * ~340 B: type,
	 * uuid, name cut to 64 chars = 192 B at most, two item ids) = 22 KB, about 721 KB. Actors are living entities, projectiles
	 * and primed TNT; nearest to the death first when there are more.
	 */
	public static final int MAX_ACTORS = 64, MAX_FRAMES = 600, MAX_BLOCK_CHANGES = 4096, MAX_EXPLOSIONS = 64;
	/** Explosions are kept within this many blocks of the anchor (the sound carries far). */
	public static final int EXPLOSION_RADIUS = 64;
	/** Block changes are kept within this many blocks of the anchor on each axis (fits a signed byte). */
	public static final int BLOCK_RADIUS = 48;

	/**
	 * One block change.
	 *
	 * @param frame    first clip frame that shows the new state, in {@code [0, frameCount]}
	 * @param dx       position relative to the anchor block (floor of the anchor)
	 * @param oldState block state id before ({@code Block.BLOCK_STATE_REGISTRY} id; unknown ids decode to air on the client)
	 * @param newState block state id after
	 */
	public record BlockChange(int frame, int dx, int dy, int dz, int oldState, int newState) {
	}

	/**
	 * One explosion.
	 *
	 * @param frame  first clip frame that shows it, in {@code [0, frameCount)}
	 * @param dx     centre relative to the anchor
	 * @param radius explosion power (about 3 for a creeper, 4 for TNT)
	 */
	public record Explosion(int frame, float dx, float dy, float dz, float radius) {
	}

	/**
	 * One recorded entity, present during frames {@code [firstFrame, firstFrame + frames)}.
	 *
	 * @param id         clip-local id (0 = victim)
	 * @param typeId     entity type id, e.g. "minecraft:player", "minecraft:zombie"
	 * @param profile    player UUID (players only), for the skin
	 * @param name       display name (player name, or custom name, or "")
	 * @param mainHand   item id held in the main hand at its last frame ("" if none), e.g. "minecraft:iron_sword"
	 * @param offHand    same for the off hand (a shield for example)
	 * @param firstFrame index of its first frame
	 * @param pos        x, y, z per frame relative to the anchor ({@code 3 * frames} values)
	 * @param rot        yaw, pitch, head yaw per frame, in 1/256 turns ({@code 3 * frames} bytes)
	 * @param flags      flag bits per frame ({@code frames} bytes)
	 * @param extra      per-frame byte, see {@code ClipExtra} (creeper swell, TNT fuse, used hand), {@code frames} bytes
	 */
	public record Actor(int id, String typeId, Optional<UUID> profile, String name, String mainHand, String offHand,
			int firstFrame, float[] pos, byte[] rot, byte[] flags, byte[] extra) {
		public int frames() {
			return this.flags.length;
		}
	}

	private static void encodeActor(ByteBuf buf, Actor a) {
		ByteBufCodecs.VAR_INT.encode(buf, a.id());
		ByteBufCodecs.STRING_UTF8.encode(buf, a.typeId());
		ByteBufCodecs.optional(UUIDUtil.STREAM_CODEC).encode(buf, a.profile());
		ByteBufCodecs.STRING_UTF8.encode(buf, a.name());
		ByteBufCodecs.STRING_UTF8.encode(buf, a.mainHand());
		ByteBufCodecs.STRING_UTF8.encode(buf, a.offHand());
		ByteBufCodecs.VAR_INT.encode(buf, a.firstFrame());
		int n = a.frames();
		ByteBufCodecs.VAR_INT.encode(buf, n);
		for (int i = 0; i < 3 * n; i++) {
			buf.writeFloat(a.pos()[i]);
		}
		buf.writeBytes(a.rot(), 0, 3 * n);
		buf.writeBytes(a.flags(), 0, n);
		buf.writeBytes(a.extra(), 0, n);
	}

	private static Actor decodeActor(ByteBuf buf, int frameCount) {
		int id = ByteBufCodecs.VAR_INT.decode(buf);
		String type = ByteBufCodecs.STRING_UTF8.decode(buf);
		Optional<UUID> profile = ByteBufCodecs.optional(UUIDUtil.STREAM_CODEC).decode(buf);
		String name = ByteBufCodecs.STRING_UTF8.decode(buf);
		String hand = ByteBufCodecs.STRING_UTF8.decode(buf);
		String offHand = ByteBufCodecs.STRING_UTF8.decode(buf);
		int first = ByteBufCodecs.VAR_INT.decode(buf);
		int n = ByteBufCodecs.VAR_INT.decode(buf);
		if (n < 0 || n > MAX_FRAMES || first < 0 || first + n > frameCount) {
			throw new IllegalArgumentException("bad clip actor frames " + first + "+" + n);
		}
		float[] pos = new float[3 * n];
		for (int i = 0; i < pos.length; i++) {
			pos[i] = buf.readFloat();
		}
		byte[] rot = new byte[3 * n];
		buf.readBytes(rot);
		byte[] flags = new byte[n];
		buf.readBytes(flags);
		byte[] extra = new byte[n];
		buf.readBytes(extra);
		return new Actor(id, type, profile, name, hand, offHand, first, pos, rot, flags, extra);
	}

	public static final StreamCodec<ByteBuf, DeathClipPayload> CODEC = StreamCodec.of((buf, p) -> {
		UUIDUtil.STREAM_CODEC.encode(buf, p.victim());
		ResourceKey.streamCodec(Registries.DIMENSION).encode(buf, p.dimension());
		buf.writeDouble(p.anchorX());
		buf.writeDouble(p.anchorY());
		buf.writeDouble(p.anchorZ());
		ByteBufCodecs.VAR_INT.encode(buf, p.sampleTicks());
		ByteBufCodecs.VAR_INT.encode(buf, p.frameCount());
		ByteBufCodecs.VAR_INT.encode(buf, p.actors().size());
		for (Actor a : p.actors()) {
			encodeActor(buf, a);
		}
		ByteBufCodecs.VAR_INT.encode(buf, p.blocks().size());
		for (BlockChange b : p.blocks()) {
			ByteBufCodecs.VAR_INT.encode(buf, b.frame());
			buf.writeByte(b.dx());
			buf.writeByte(b.dy());
			buf.writeByte(b.dz());
			ByteBufCodecs.VAR_INT.encode(buf, b.oldState());
			ByteBufCodecs.VAR_INT.encode(buf, b.newState());
		}
		ByteBufCodecs.VAR_INT.encode(buf, p.explosions().size());
		for (Explosion x : p.explosions()) {
			ByteBufCodecs.VAR_INT.encode(buf, x.frame());
			buf.writeFloat(x.dx());
			buf.writeFloat(x.dy());
			buf.writeFloat(x.dz());
			buf.writeFloat(x.radius());
		}
	}, buf -> {
		UUID victim = UUIDUtil.STREAM_CODEC.decode(buf);
		ResourceKey<Level> dim = ResourceKey.streamCodec(Registries.DIMENSION).decode(buf);
		double x = buf.readDouble(), y = buf.readDouble(), z = buf.readDouble();
		int sample = ByteBufCodecs.VAR_INT.decode(buf);
		int frames = ByteBufCodecs.VAR_INT.decode(buf);
		int count = ByteBufCodecs.VAR_INT.decode(buf);
		if (frames < 1 || frames > MAX_FRAMES || count < 0 || count > MAX_ACTORS || sample < 1) {
			throw new IllegalArgumentException("bad clip header");
		}
		List<Actor> actors = new ArrayList<>(count);
		for (int i = 0; i < count; i++) {
			actors.add(decodeActor(buf, frames));
		}
		int blockCount = ByteBufCodecs.VAR_INT.decode(buf);
		if (blockCount < 0 || blockCount > MAX_BLOCK_CHANGES) {
			throw new IllegalArgumentException("bad clip block count " + blockCount);
		}
		List<BlockChange> blocks = new ArrayList<>(blockCount);
		for (int i = 0; i < blockCount; i++) {
			int frame = ByteBufCodecs.VAR_INT.decode(buf);
			if (frame < 0 || frame > frames) {
				throw new IllegalArgumentException("bad clip block frame " + frame);
			}
			int dx = buf.readByte(), dy = buf.readByte(), dz = buf.readByte();
			int oldState = ByteBufCodecs.VAR_INT.decode(buf), newState = ByteBufCodecs.VAR_INT.decode(buf);
			if (oldState < 0 || newState < 0) {
				throw new IllegalArgumentException("bad clip block state");
			}
			blocks.add(new BlockChange(frame, dx, dy, dz, oldState, newState));
		}
		int explosionCount = ByteBufCodecs.VAR_INT.decode(buf);
		if (explosionCount < 0 || explosionCount > MAX_EXPLOSIONS) {
			throw new IllegalArgumentException("bad clip explosion count " + explosionCount);
		}
		List<Explosion> explosions = new ArrayList<>(explosionCount);
		for (int i = 0; i < explosionCount; i++) {
			int frame = ByteBufCodecs.VAR_INT.decode(buf);
			float dx = buf.readFloat(), dy = buf.readFloat(), dz = buf.readFloat(), radius = buf.readFloat();
			// written as !(a <= b) so NaN is rejected too
			if (frame < 0 || frame >= frames || !(radius >= 0 && radius <= 64) || !(Math.abs(dx) <= 1.0e6f)
					|| !(Math.abs(dy) <= 1.0e6f) || !(Math.abs(dz) <= 1.0e6f)) {
				throw new IllegalArgumentException("bad clip explosion");
			}
			explosions.add(new Explosion(frame, dx, dy, dz, radius));
		}
		return new DeathClipPayload(victim, dim, x, y, z, sample, frames, List.copyOf(actors), List.copyOf(blocks),
				List.copyOf(explosions));
	});

	@Override
	public Type<DeathClipPayload> type() {
		return TYPE;
	}
}
