package fr.classroomrun.client.clip;

import fr.classroomrun.net.DeathClipPayload.BlockChange;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class ClipBlocksTest {
	@Test
	void stateAtFrameIncludingSeekBack() {
		// one position: A -> B at frame 3, B -> C at frame 7; another: X -> Y at frame 5
		ClipBlocks b = new ClipBlocks(List.of(new BlockChange(3, 0, 0, 0, 10, 11), new BlockChange(5, -1, 2, 3, 20, 21),
				new BlockChange(7, 0, 0, 0, 11, 12)));
		assertEquals(2, b.positions.length);
		ClipBlocks.Pos p = b.positions[0];
		assertEquals(10, p.stateAt(0));
		assertEquals(10, p.stateAt(2));
		assertEquals(11, p.stateAt(3));
		assertEquals(11, p.stateAt(6));
		assertEquals(12, p.stateAt(7));
		assertEquals(12, p.stateAt(99));
		assertEquals(10, p.stateAt(1)); // seeking back
		ClipBlocks.Pos q = b.positions[1];
		assertEquals(-1, q.dx());
		assertEquals(20, q.stateAt(4));
		assertEquals(21, q.stateAt(5));
	}
}
