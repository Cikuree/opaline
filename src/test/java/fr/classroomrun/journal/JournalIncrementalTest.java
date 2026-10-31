package fr.classroomrun.journal;

import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.Instant;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class JournalIncrementalTest {
	private static final Instant AT = Instant.parse("2026-10-30T18:00:00Z");

	@Test
	void currentRunIsReadIncrementallyAndPartialLinesWait(@TempDir Path dir) throws Exception {
		JournalStore store = new JournalStore(dir);
		Path file = store.journalFile(5);
		try (RunJournal j = new RunJournal(file)) {
			j.append(JournalEvents.event(JournalEvents.START, AT, 0));
			j.append(JournalEvents.event(JournalEvents.GO, AT, 0));
			assertEquals(0, store.run(5).summary().deaths());
			JsonObject death = JournalEvents.event(JournalEvents.DEATH, AT, 4000);
			// half of the line is on disk: not counted yet
			String line = death + "\n";
			Files.writeString(file, line.substring(0, 20), StandardOpenOption.APPEND);
			assertEquals(0, store.run(5).summary().deaths());
			Files.writeString(file, line.substring(20), StandardOpenOption.APPEND);
			assertEquals(1, store.run(5).summary().deaths());
			assertEquals(4000, store.run(5).summary().durationMs());
		}
	}

	@Test
	void validButUnexpectedLineIsSkippedNotFatal() {
		var p = JournalReader.parse(1, List.of(
				"{\"e\":\"damage\",\"amount\":{\"a\":1}}", // wrong type in a field: not fatal
				"{\"e\":\"go\",\"t\":\"x\"}",
				JournalEvents.event(JournalEvents.START, AT, 0).toString()));
		assertEquals("2026-10-30T18:00:00Z", p.summary().startedAt());
		assertEquals(0, p.summary().deaths());
	}
}
