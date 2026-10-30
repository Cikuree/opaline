package fr.classroomrun.journal;

import com.google.gson.Gson;
import com.google.gson.JsonObject;

import java.io.BufferedWriter;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;

/**
 * Append-only journal of one run: one JSON object per line, flushed after each line, never rewritten. A line cut by a
 * crash stays at the end of the file and is skipped by {@link JournalReader}.
 *
 * <p>Writes are synchronous on the calling (server) thread: events are rare (hits, heals, deaths, splits), each line
 * is a few hundred bytes and goes to the OS page cache on flush, so no executor is needed.
 */
public final class RunJournal implements AutoCloseable {
	private static final Gson GSON = new com.google.gson.GsonBuilder().serializeNulls().create(); // the contract has explicit nulls

	private final BufferedWriter writer;

	public RunJournal(Path file) throws IOException {
		Files.createDirectories(file.getParent());
		boolean needsNewline = endsMidLine(file);
		this.writer = Files.newBufferedWriter(file, StandardCharsets.UTF_8, StandardOpenOption.CREATE, StandardOpenOption.APPEND);
		if (needsNewline) {
			this.writer.write('\n'); // never glue a new line to a line cut by a crash
		}
	}

	private static boolean endsMidLine(Path file) throws IOException {
		if (!Files.exists(file) || Files.size(file) == 0) {
			return false;
		}
		try (RandomAccessFile f = new RandomAccessFile(file.toFile(), "r")) {
			f.seek(f.length() - 1);
			return f.read() != '\n';
		}
	}

	public void append(JsonObject event) throws IOException {
		this.writer.write(GSON.toJson(event));
		this.writer.write('\n');
		this.writer.flush();
	}

	@Override
	public void close() throws IOException {
		this.writer.close();
	}
}
