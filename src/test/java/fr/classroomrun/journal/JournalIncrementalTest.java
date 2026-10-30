package fr.classroomrun.journal;

import com.google.gson.JsonObject;
import fr.classroomrun.http.StatsHttpServer;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.Instant;
import java.util.List;
import java.util.Optional;
import java.util.Set;

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
			death.add("replay", JournalEvents.replay("a.mcpr", 1));
			// half of the line is on disk: not counted yet
			String line = death + "\n";
			Files.writeString(file, line.substring(0, 20), StandardOpenOption.APPEND);
			assertEquals(0, store.run(5).summary().deaths());
			Files.writeString(file, line.substring(20), StandardOpenOption.APPEND);
			assertEquals(1, store.run(5).summary().deaths());
			assertEquals(Set.of("a.mcpr"), store.replayPaths());
			assertEquals(4000, store.run(5).summary().durationMs());
		}
	}

	@Test
	void validButUnexpectedLineIsSkippedNotFatal() {
		var p = JournalReader.parse(1, List.of(
				"{\"e\":\"death\",\"replay\":{\"path\":{\"a\":1}}}", // path is an object: getAsString throws
				"{\"e\":\"go\",\"t\":\"x\"}",
				JournalEvents.event(JournalEvents.START, AT, 0).toString()));
		assertEquals("2026-10-30T18:00:00Z", p.summary().startedAt());
		assertEquals(0, p.summary().deaths());
	}

	@Test
	void pendingReplayAnswers503AndNeverStreams(@TempDir Path tmp) throws Exception {
		Path f = tmp.resolve("a.mcpr");
		Files.writeString(f, "data");
		StatsHttpServer http = new StatsHttpServer("127.0.0.1", 0);
		http.setReplayResolver(raw -> Optional.of(f));
		http.setReplayPending("busy"::equals);
		http.start();
		try {
			HttpClient c = HttpClient.newHttpClient();
			String base = "http://127.0.0.1:" + http.port() + "/replay?path=";
			assertEquals(503, c.send(HttpRequest.newBuilder(URI.create(base + "busy")).build(), HttpResponse.BodyHandlers.discarding()).statusCode());
			assertEquals(200, c.send(HttpRequest.newBuilder(URI.create(base + "done")).build(), HttpResponse.BodyHandlers.discarding()).statusCode());
		} finally {
			http.stop();
		}
	}
}
