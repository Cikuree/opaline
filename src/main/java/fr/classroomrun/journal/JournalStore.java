package fr.classroomrun.journal;

import org.jspecify.annotations.Nullable;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Reads {@code runs/<n>/journal.jsonl}. The index is rebuilt from the journals; a parsed journal is cached by
 * (size, mtime) so only the files that changed are read again. Thread-safe (HTTP and network threads).
 */
public final class JournalStore {
	public static final String FILE = "journal.jsonl";

	private record Cached(long size, long mtime, JournalReader.Parsed parsed) {
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

	public synchronized JournalReader.@Nullable Parsed run(int run) {
		Path file = journalFile(run);
		try {
			long size = Files.size(file);
			long mtime = Files.getLastModifiedTime(file).toMillis();
			Cached c = this.cache.get(run);
			if (c != null && c.size() == size && c.mtime() == mtime) {
				return c.parsed();
			}
			List<String> lines = new ArrayList<>();
			// InputStreamReader replaces malformed bytes (a multi-byte character cut by a crash) instead of failing
			try (BufferedReader r = new BufferedReader(new InputStreamReader(Files.newInputStream(file), StandardCharsets.UTF_8))) {
				String line;
				while ((line = r.readLine()) != null) {
					lines.add(line);
				}
			}
			JournalReader.Parsed parsed = JournalReader.parse(run, lines);
			this.cache.put(run, new Cached(size, mtime, parsed));
			return parsed;
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

	/** Every replay path written in any journal: the only paths the HTTP route may serve. */
	public synchronized Set<String> replayPaths() {
		Set<String> all = new HashSet<>();
		for (int n : runNumbers()) {
			JournalReader.Parsed p = run(n);
			if (p != null) {
				all.addAll(p.replayPaths());
			}
		}
		return all;
	}
}
