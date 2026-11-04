package fr.classroomrun.clip;

import fr.classroomrun.net.DeathClipPayload;
import fr.classroomrun.net.DeathClipPayload.Actor;
import fr.classroomrun.net.DeathClipPayload.BlockChange;
import fr.classroomrun.net.DeathClipPayload.Explosion;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Optional;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

class DeathClipPayloadCodecTest {
	private static DeathClipPayload payload(int frameCount, int firstFrame) {
		return payload(frameCount, firstFrame, List.of(new BlockChange(2, -48, 7, 48, 20000, 0),
				new BlockChange(frameCount, 1, -2, 3, 0, 1)));
	}

	private static DeathClipPayload payload(int frameCount, int firstFrame, List<BlockChange> blocks) {
		return payload(frameCount, firstFrame, blocks, List.of(new Explosion(0, 1.5f, -2, 3, 3f)));
	}

	private static DeathClipPayload payload(int frameCount, int firstFrame, List<BlockChange> blocks,
			List<Explosion> explosions) {
		UUID victim = UUID.randomUUID();
		Actor actor = new Actor(0, "minecraft:player", Optional.of(victim), "Alice", "minecraft:iron_sword",
				"minecraft:shield", firstFrame, new float[] {1, 2, 3, 4, 5, 6}, new byte[] {1, 2, 3, 4, 5, 6},
				new byte[] {1, (byte) 0x88}, new byte[] {7, (byte) 0x80});
		return new DeathClipPayload(victim, ResourceKey.create(Registries.DIMENSION, Identifier.parse("minecraft:overworld")),
				10.5, 64, -3.25, 2, frameCount, List.of(actor), blocks, explosions);
	}

	@Test
	void roundTrip() {
		DeathClipPayload in = payload(5, 3);
		ByteBuf buf = Unpooled.buffer();
		DeathClipPayload.CODEC.encode(buf, in);
		DeathClipPayload out = DeathClipPayload.CODEC.decode(buf);
		assertEquals(0, buf.readableBytes());
		assertEquals(in.victim(), out.victim());
		assertEquals(in.dimension(), out.dimension());
		assertEquals(in.anchorX(), out.anchorX());
		assertEquals(in.anchorY(), out.anchorY());
		assertEquals(in.anchorZ(), out.anchorZ());
		assertEquals(in.sampleTicks(), out.sampleTicks());
		assertEquals(in.frameCount(), out.frameCount());
		Actor a = in.actors().get(0), b = out.actors().get(0);
		assertEquals(in.blocks(), out.blocks());
		assertEquals(in.explosions(), out.explosions());
		assertEquals(a.offHand(), b.offHand());
		assertArrayEquals(a.extra(), b.extra());
		assertEquals(a.typeId(), b.typeId());
		assertEquals(a.profile(), b.profile());
		assertEquals(a.name(), b.name());
		assertEquals(a.mainHand(), b.mainHand());
		assertEquals(a.firstFrame(), b.firstFrame());
		assertArrayEquals(a.pos(), b.pos());
		assertArrayEquals(a.rot(), b.rot());
		assertArrayEquals(a.flags(), b.flags());
	}

	@Test
	void rejectsActorOutsideFrameCount() {
		ByteBuf buf = Unpooled.buffer();
		DeathClipPayload.CODEC.encode(buf, payload(5, 4)); // 4 + 2 frames > 5
		assertThrows(IllegalArgumentException.class, () -> DeathClipPayload.CODEC.decode(buf));
	}

	@Test
	void rejectsBadExplosions() {
		for (List<Explosion> bad : List.of(List.of(new Explosion(5, 0, 0, 0, 3f)), // frame >= frameCount
				List.of(new Explosion(-1, 0, 0, 0, 3f)), List.of(new Explosion(0, 0, 0, 0, Float.NaN)),
				List.of(new Explosion(0, 0, 0, 0, 65f)), List.of(new Explosion(0, Float.POSITIVE_INFINITY, 0, 0, 3f)),
				java.util.Collections.nCopies(DeathClipPayload.MAX_EXPLOSIONS + 1, new Explosion(0, 0, 0, 0, 3f)))) {
			ByteBuf buf = Unpooled.buffer();
			DeathClipPayload.CODEC.encode(buf, payload(5, 3, List.of(), bad));
			assertThrows(IllegalArgumentException.class, () -> DeathClipPayload.CODEC.decode(buf));
		}
	}

	/** The documented worst case: every cap reached at once stays under the 1 MiB custom payload limit. */
	@Test
	void worstCaseStaysUnderOneMebibyte() {
		int frames = DeathClipPayload.MAX_FRAMES;
		String longName = "\u20ac".repeat(64); // 3 bytes per char in UTF-8
		List<Actor> actors = new java.util.ArrayList<>();
		for (int i = 0; i < DeathClipPayload.MAX_ACTORS; i++) {
			actors.add(new Actor(i, "minecraft:player", Optional.of(UUID.randomUUID()), longName, "minecraft:netherite_sword",
					"minecraft:totem_of_undying", 0, new float[3 * frames], new byte[3 * frames], new byte[frames],
					new byte[frames]));
		}
		List<BlockChange> blocks = java.util.Collections.nCopies(DeathClipPayload.MAX_BLOCK_CHANGES,
				new BlockChange(frames, -48, -48, -48, 2_000_000, 2_000_000));
		List<Explosion> booms = java.util.Collections.nCopies(DeathClipPayload.MAX_EXPLOSIONS,
				new Explosion(frames - 1, -64f, -64f, -64f, 64f));
		DeathClipPayload worst = new DeathClipPayload(UUID.randomUUID(),
				ResourceKey.create(Registries.DIMENSION, Identifier.parse("minecraft:overworld")), 0, 0, 0, 2, frames, actors,
				blocks, booms);
		ByteBuf buf = Unpooled.buffer();
		DeathClipPayload.CODEC.encode(buf, worst);
		assertTrue(buf.readableBytes() < 1 << 20, "payload is " + buf.readableBytes() + " bytes");
		assertEquals(worst.actors().size(), DeathClipPayload.CODEC.decode(buf).actors().size());
	}

	@Test
	void rejectsBadBlocks() {
		for (List<BlockChange> bad : List.of(List.of(new BlockChange(6, 0, 0, 0, 1, 2)), // frame > frameCount
				java.util.Collections.nCopies(DeathClipPayload.MAX_BLOCK_CHANGES + 1, new BlockChange(0, 0, 0, 0, 1, 2)),
				List.of(new BlockChange(0, 0, 0, 0, -1, 2)))) {
			ByteBuf buf = Unpooled.buffer();
			DeathClipPayload.CODEC.encode(buf, payload(5, 3, bad));
			assertThrows(IllegalArgumentException.class, () -> DeathClipPayload.CODEC.decode(buf));
		}
	}
}
