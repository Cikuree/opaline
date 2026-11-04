package fr.classroomrun.clip;

import fr.classroomrun.net.DeathClipPayload.Explosion;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class ExplosionLogTest {
	private static final String OW = "minecraft:overworld";

	@Test
	void framesMatchTheSampleCounterAndPositionsAreRelative() {
		ExplosionLog log = new ExplosionLog(8);
		for (int i = 0; i < 5; i++) {
			log.nextSample();
		}
		log.add(OW, 12, 70, -4, 3f); // after 5 samples
		log.nextSample(); // 6 samples, a clip of 4 frames starts at sample 2
		List<Explosion> out = log.build(OW, 10, 64, -4, 64, 4, 8);
		assertEquals(List.of(new Explosion(3, 2f, 6f, 0f, 3f)), out);
	}

	@Test
	void dropsOldOtherDimensionAndFarOnes() {
		ExplosionLog log = new ExplosionLog(8);
		log.add(OW, 0, 0, 0, 3f); // before the clip
		for (int i = 0; i < 4; i++) {
			log.nextSample();
		}
		log.add("minecraft:the_nether", 0, 0, 0, 3f);
		log.add(OW, 100, 0, 0, 3f);
		log.add(OW, 1, 0, 0, 3f);
		List<Explosion> out = log.build(OW, 0, 0, 0, 64, 2, 8);
		assertEquals(1, out.size());
		assertEquals(1f, out.get(0).dx());
		assertEquals(1, out.get(0).frame()); // after the last sample: clamped onto the last frame
	}

	@Test
	void capacityAndMaxKeepTheLatest() {
		ExplosionLog log = new ExplosionLog(3);
		for (int i = 0; i < 5; i++) {
			log.add(OW, i, 0, 0, 3f);
		}
		assertEquals(3, log.size());
		log.nextSample();
		List<Explosion> out = log.build(OW, 0, 0, 0, 64, 1, 2);
		assertEquals(List.of(3f, 4f), out.stream().map(Explosion::dx).toList());
		log.clear();
		assertEquals(0, log.size());
	}
}
