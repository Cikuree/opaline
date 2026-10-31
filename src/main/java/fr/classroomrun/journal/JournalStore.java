package fr.classroomrun.journal;

import org.jspecify.annotations.Nullable;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.SeekableByteChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Reads {@code runs/<n>/journal.jsonl}. The index is rebuilt from the journals; a parsed journal is cached by
 * read offset, so a growing journal (the current run) is read incrementally, never again from the start. Thread-safe (HTTP and network threads).
 */
public final class JournalStore {
	public static final String FILE = "journal.jsonl";

	private static final class Cached {
		final JournalReader.Accumulator acc;
		long offset;
		JournalReader.Parsed parsed;

		Cached(JournalReader.Accumulator acc) {
			this.acc = acc;
		}
	}

	private final Path runsDir;
	private final Map<Integer, Cached> cache = new HashMap<>();

	public JournalStore(Path runsDir) {
		this.runsDir = runsDir;
	}

	public Path journalFile(int run) {
		return this.runsDir.resolve(Integer.toString(run)).resolve(FILE);
	}

	private List<Integer> runNumbers() {
		List<Integer> runs = new ArrayList<>();
		if (!Files.isDirectory(this.runsDir)) {
			return runs;
		}
		try (DirectoryStream<Path> dirs = Files.newDirectoryStream(this.runsDir)) {
			for (Path dir : dirs) {
				try {
					int n = Integer.parseInt(dir.getFileName().toString());
					if (n >= 0 && Files.isRegularFile(dir.resolve(FILE))) {
						runs.add(n);
					}
				} catch (NumberFormatException e) {
					// not a run directory
				}
			}
		} catch (IOException e) {
			// unreadable directory: empty index
		}
		return runs;
	}

	/**
	 * Reads only what was appended since the last call (the journal is append-only), and only complete lines: a line
	 * being written, or cut by a crash, stays unread. The result is an immutable snapshot, cached until the file grows.
	 */
	public synchronized JournalReader.@Nullable Parsed run(int run) {
		Path file = journalFile(run);
		try {
			long size = Files.size(file);
			Cached c = this.cache.get(run);
			if (c == null || size < c.offset) { // new or replaced file
				c = new Cached(new JournalReader.Accumulator(run));
				this.cache.put(run, c);
			}
			if (size > c.offset) {
				try (SeekableByteChannel ch = Files.newByteChannel(file)) {
					ch.position(c.offset);
					ByteBuffer buf = ByteBuffer.allocate((int) Math.min(size - c.offset, 64L << 20));
					while (buf.hasRemaining() && ch.read(buf) > 0) {
						// fill
					}
					byte[] bytes = Arrays.copyOf(buf.array(), buf.position());
					int end = bytes.length;
					while (end > 0 && bytes[end - 1] != (byte) 10) {
						end--;
					}
					// malformed bytes are replaced rather than failing (multi-byte character cut by a crash)
					for (String line : new String(bytes, 0, end, StandardCharsets.UTF_8).split("\n")) {
						c.acc.add(line);
					}
					c.offset += end;
					c.parsed = null;
				}
			}
			if (c.parsed == null) {
				c.parsed = c.acc.snapshot();
			}
			return c.parsed;
		} catch (IOException e) {
			this.cache.remove(run);
			return null;
		}
	}

	/** Summaries, most recent run first. */
	public synchronized List<RunSummary> summaries() {
		List<RunSummary> out = new ArrayList<>();
		for (int n : runNumbers()) {
			JournalReader.Parsed p = run(n);
			if (p != null) {
				out.add(p.summary());
			}
		}
		out.sort(Comparator.comparingInt(RunSummary::run).reversed());
		return out;
	}

	/** Index JSON (CONTRACT.md "Index"), same generator as {@code HistoryPayload}. */
	public String indexJson(int maxChars) {
		return HistoryJson.index(summaries(), maxChars);
	}

	/** Detail JSON of one run (CONTRACT.md "Détail"); an unknown run gives an empty {@code deaths}. */
	public String detailJson(int run, int maxChars) {
		JournalReader.Parsed p = run(run);
		return p == null ? HistoryJson.detail(run, "", List.of(), maxChars)
				: HistoryJson.detail(run, p.summary().startedAt(), p.deaths(), maxChars);
	}
}
