package fr.classroomrun.clip;

import fr.classroomrun.net.DeathClipPayload;
import fr.classroomrun.net.DeathClipPayload.Actor;
import fr.classroomrun.net.DeathClipPayload.BlockChange;
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
		UUID victim = UUID.randomUUID();
		Actor actor = new Actor(0, "minecraft:player", Optional.of(victim), "Alice", "minecraft:iron_sword", firstFrame,
				new float[] {1, 2, 3, 4, 5, 6}, new byte[] {1, 2, 3, 4, 5, 6}, new byte[] {1, 8});
		return new DeathClipPayload(victim, ResourceKey.create(Registries.DIMENSION, Identifier.parse("minecraft:overworld")),
				10.5, 64, -3.25, 2, frameCount, List.of(actor), blocks);
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
