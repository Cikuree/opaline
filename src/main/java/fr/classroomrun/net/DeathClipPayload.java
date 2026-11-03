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
 */
public record DeathClipPayload(UUID victim, ResourceKey<Level> dimension, double anchorX, double anchorY, double anchorZ,
		int sampleTicks, int frameCount, List<Actor> actors) implements CustomPacketPayload {
	public static final Type<DeathClipPayload> TYPE = new Type<>(ClassroomRunNetworking.id("death_clip"));

	/** Per-frame flag bits of {@link Actor#flags()}. */
	public static final int ON_GROUND = 1, CROUCHING = 1 << 1, SPRINTING = 1 << 2, SWING = 1 << 3, HURT = 1 << 4,
			FALL_FLYING = 1 << 5;
	/** Hard caps so the payload stays far below the 1 MiB custom payload limit. */
	public static final int MAX_ACTORS = 32, MAX_FRAMES = 600;

	/**
	 * One recorded entity, present during frames {@code [firstFrame, firstFrame + frames)}.
	 *
	 * @param id         clip-local id (0 = victim)
	 * @param typeId     entity type id, e.g. "minecraft:player", "minecraft:zombie"
	 * @param profile    player UUID (players only), for the skin
	 * @param name       display name (player name, or custom name, or "")
	 * @param mainHand   item id held in the main hand at its last frame ("" if none), e.g. "minecraft:iron_sword"
	 * @param firstFrame index of its first frame
	 * @param pos        x, y, z per frame relative to the anchor ({@code 3 * frames} values)
	 * @param rot        yaw, pitch, head yaw per frame, in 1/256 turns ({@code 3 * frames} bytes)
	 * @param flags      flag bits per frame ({@code frames} bytes)
	 */
	public record Actor(int id, String typeId, Optional<UUID> profile, String name, String mainHand, int firstFrame,
			float[] pos, byte[] rot, byte[] flags) {
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
		ByteBufCodecs.VAR_INT.encode(buf, a.firstFrame());
		int n = a.frames();
		ByteBufCodecs.VAR_INT.encode(buf, n);
		for (int i = 0; i < 3 * n; i++) {
			buf.writeFloat(a.pos()[i]);
		}
		buf.writeBytes(a.rot(), 0, 3 * n);
		buf.writeBytes(a.flags(), 0, n);
	}

	private static Actor decodeActor(ByteBuf buf, int frameCount) {
		int id = ByteBufCodecs.VAR_INT.decode(buf);
		String type = ByteBufCodecs.STRING_UTF8.decode(buf);
		Optional<UUID> profile = ByteBufCodecs.optional(UUIDUtil.STREAM_CODEC).decode(buf);
		String name = ByteBufCodecs.STRING_UTF8.decode(buf);
		String hand = ByteBufCodecs.STRING_UTF8.decode(buf);
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
		return new Actor(id, type, profile, name, hand, first, pos, rot, flags);
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
		return new DeathClipPayload(victim, dim, x, y, z, sample, frames, List.copyOf(actors));
	});

	@Override
	public Type<DeathClipPayload> type() {
		return TYPE;
	}
}
