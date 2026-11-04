package fr.classroomrun.clip;

import fr.classroomrun.net.DeathClipPayload.BlockChange;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class BlockLogTest {
	@Test
	void framesFollowSamplesAndAreRelativeToAnchor() {
		BlockLog log = new BlockLog(100);
		log.add("o", 10, 64, 10, 1, 2); // before any sample: shown from frame 0
		log.nextSample();
		log.nextSample();
		log.add("o", 12, 60, 8, 2, 3); // after 2 samples: shown from frame 2
		log.nextSample(); // final sample, 3 frames
		assertEquals(List.of(new BlockChange(0, 0, 0, 0, 1, 2), new BlockChange(2, 2, -4, -2, 2, 3)),
				log.build("o", 10, 64, 10, 48, 3, 10));
	}

	@Test
	void dropsOutOfWindowRangeDimensionAndNoops() {
		BlockLog log = new BlockLog(100);
		log.add("o", 0, 0, 0, 1, 2); // older than the clip
		log.nextSample();
		log.nextSample();
		log.add("o", 0, 0, 0, 5, 5); // no-op
		log.add("o", 49, 0, 0, 1, 2); // too far
		log.add("n", 0, 0, 0, 1, 2); // other dimension
		log.add("o", 1, 0, 0, 1, 2);
		assertEquals(List.of(new BlockChange(1, 1, 0, 0, 1, 2)), log.build("o", 0, 0, 0, 48, 1, 10));
	}

	@Test
	void capsDropOldest() {
		BlockLog log = new BlockLog(3);
		for (int i = 0; i < 5; i++) {
			log.add("o", i, 0, 0, 0, 1);
		}
		assertEquals(3, log.size());
		log.nextSample();
		List<BlockChange> out = log.build("o", 0, 0, 0, 48, 1, 2);
		assertEquals(List.of(3, 4), out.stream().map(BlockChange::dx).toList());
	}
}
