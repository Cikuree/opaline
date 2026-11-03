package fr.classroomrun.clip;

import fr.classroomrun.clip.ClipBuffer.Entry;
import fr.classroomrun.clip.ClipBuffer.Info;
import fr.classroomrun.net.DeathClipPayload;
import org.junit.jupiter.api.Test;

import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.*;

class ClipTest {
	private static final UUID VICTIM = new UUID(0, 1);
	private static final Info INFO = new Info("minecraft:zombie", Optional.empty(), "", "");

	private static Entry at(double x, double y, double z) {
		return new Entry("minecraft:overworld", INFO, x, y, z, 90.0F, -45.0F, 180.0F, DeathClipPayload.ON_GROUND);
	}

	private static Map<UUID, Entry> frame(Object... idAndEntry) {
		Map<UUID, Entry> m = new HashMap<>();
		for (int i = 0; i < idAndEntry.length; i += 2) {
			m.put((UUID) idAndEntry[i], (Entry) idAndEntry[i + 1]);
		}
		return m;
	}

	@Test
	void ringBufferKeepsTheLastFramesAndDropsOldEntities() {
		ClipBuffer b = new ClipBuffer(3);
		UUID old = new UUID(0, 9);
		b.add(frame(VICTIM, at(0, 0, 0), old, at(1, 0, 0)));
		for (int i = 1; i <= 3; i++) {
			b.add(frame(VICTIM, at(i, 0, 0)));
		}
		assertEquals(3, b.size());
		ClipBuffer.Clip clip = b.build(VICTIM, "minecraft:overworld", 3, 0, 0, 48);
		assertEquals(3, clip.frameCount());
		assertEquals(1, clip.actors().size()); // the entity only seen in the evicted frame is gone
		assertEquals(-2.0F, clip.actors().get(0).pos()[0]); // oldest kept frame x = 1, anchor 3
	}

	@Test
	void positionsAreRelativeToTheAnchorAndAnglesPackedInTurns() {
		ClipBuffer b = new ClipBuffer(10);
		b.add(frame(VICTIM, at(11, 65, -5)));
		DeathClipPayload.Actor a = b.build(VICTIM, "minecraft:overworld", 10, 64, -4, 48).actors().get(0);
		assertArrayEquals(new float[] {1, 1, -1}, a.pos());
		assertArrayEquals(new byte[] {64, -32, -128}, a.rot()); // 90 deg, -45 deg, 180 deg
		assertEquals(DeathClipPayload.ON_GROUND, a.flags()[0]);
		assertEquals(0, a.id());
	}

	@Test
	void victimFirstThenNearestAndCapped() {
		ClipBuffer b = new ClipBuffer(10);
		Map<UUID, Entry> f = new HashMap<>();
		f.put(VICTIM, at(0, 0, 0));
		for (int i = 0; i < 40; i++) {
			f.put(new UUID(1, i), at(40 - i, 0, 0)); // i = 39 is the nearest (x = 1)
		}
		f.put(new UUID(2, 0), at(100, 0, 0)); // too far
		b.add(f);
		var actors = b.build(VICTIM, "minecraft:overworld", 0, 0, 0, 48).actors();
		assertEquals(DeathClipPayload.MAX_ACTORS, actors.size());
		assertArrayEquals(new float[] {0, 0, 0}, actors.get(0).pos());
		assertEquals(1.0F, actors.get(1).pos()[0]);
		assertEquals(2.0F, actors.get(2).pos()[0]);
	}

	@Test
	void reappearingActorKeepsItsLastContiguousWindow() {
		ClipBuffer b = new ClipBuffer(10);
		UUID z = new UUID(5, 5);
		b.add(frame(VICTIM, at(0, 0, 0), z, at(1, 0, 0)));
		b.add(frame(VICTIM, at(0, 0, 0)));
		b.add(frame(VICTIM, at(0, 0, 0), z, at(2, 0, 0)));
		b.add(frame(VICTIM, at(0, 0, 0), z, at(3, 0, 0)));
		DeathClipPayload.Actor a = b.build(VICTIM, "minecraft:overworld", 0, 0, 0, 48).actors().get(1);
		assertEquals(2, a.firstFrame());
		assertEquals(2, a.frames());
	}

	@Test
	void emptyBufferOrMissingVictimGivesNoClip() {
		ClipBuffer b = new ClipBuffer(10);
		assertNull(b.build(VICTIM, "minecraft:overworld", 0, 0, 0, 48));
		b.add(frame(new UUID(7, 7), at(0, 0, 0)));
		assertNull(b.build(VICTIM, "minecraft:overworld", 0, 0, 0, 48));
	}

	@Test
	void allReadyStartsAShortCountdown() {
		ClipWait w = new ClipWait(120, 3600, 40);
		for (int i = 0; i < 100; i++) {
			assertFalse(w.tick(2, 1));
		}
		for (int i = 0; i < 40; i++) {
			assertFalse(w.tick(2, 2));
		}
		assertTrue(w.tick(2, 2));
	}

	@Test
	void maxWaitForcesTheResetAndNobodyOnlineUsesTheMinimum() {
		ClipWait forced = new ClipWait(120, 200, 40);
		for (int i = 0; i < 199; i++) {
			assertFalse(forced.tick(3, 0));
		}
		assertTrue(forced.tick(3, 0));
		ClipWait none = new ClipWait(120, 3600, 40);
		for (int i = 0; i < 119; i++) {
			assertFalse(none.tick(0, 0));
		}
		assertTrue(none.tick(0, 0));
		assertEquals(3, new ClipWait(0, 60, 0).secondsLeft());
	}
}
