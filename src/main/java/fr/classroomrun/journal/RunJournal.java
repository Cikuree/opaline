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

	private final Path file;
	private BufferedWriter writer;

	public RunJournal(Path file) throws IOException {
		this.file = file;
		Files.createDirectories(file.getParent());
		open();
	}

	private void open() throws IOException {
		boolean needsNewline = endsMidLine(this.file);
		this.writer = Files.newBufferedWriter(this.file, StandardCharsets.UTF_8, StandardOpenOption.CREATE, StandardOpenOption.APPEND);
		if (needsNewline) {
			this.writer.write("\n"); // never glue a new line to a line cut by a crash
			this.writer.flush();
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

	/**
	 * One write of the whole line, then a flush. No fsync on purpose: a process crash loses nothing (the data is in the
	 * OS cache), only a power loss or OS crash can lose the last lines, which the reader tolerates.
	 * After an I/O error the stream is closed and reopened (a half-written line is then ended by the newline rule).
	 */
	public void append(JsonObject event) throws IOException {
		try {
			this.writer.write(GSON.toJson(event) + "\n");
			this.writer.flush();
		} catch (IOException e) {
			try {
				this.writer.close();
			} catch (IOException ignored) {
				// already broken
			}
			open();
			throw e;
		}
	}

	@Override
	public void close() throws IOException {
		this.writer.close();
	}
}
