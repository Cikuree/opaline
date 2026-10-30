package fr.classroomrun.stats;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonParseException;
import org.slf4j.Logger;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

/** Reads and writes {@link RunStats} as JSON. Writes are atomic (temporary file then move). */
public final class StatsStore {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	private StatsStore() {
	}

	/** Missing file: fresh stats. Corrupted file: moved to {@code <name>.bak}, WARN log, fresh stats. */
	public static RunStats load(Path file, Logger log) {
		if (!Files.exists(file)) {
			return new RunStats();
		}
		try {
			RunStats stats = GSON.fromJson(Files.readString(file, StandardCharsets.UTF_8), RunStats.class);
			if (stats == null) {
				throw new JsonParseException("empty file");
			}
			return stats;
		} catch (IOException | JsonParseException e) {
			Path backup = file.resolveSibling(file.getFileName() + ".bak");
			log.warn("Stats file {} is unreadable ({}), saving it as {} and starting from scratch", file, e.getMessage(), backup);
			try {
				Files.move(file, backup, StandardCopyOption.REPLACE_EXISTING);
			} catch (IOException moveError) {
				log.warn("Could not back up {}: {}", file, moveError.getMessage());
			}
			return new RunStats();
		}
	}

	public static void save(Path file, RunStats stats) throws IOException {
		Files.createDirectories(file.toAbsolutePath().getParent());
		Path tmp = file.resolveSibling(file.getFileName() + ".tmp");
		Files.writeString(tmp, GSON.toJson(stats), StandardCharsets.UTF_8);
		try {
			Files.move(tmp, file, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
		} catch (AtomicMoveNotSupportedException e) {
			Files.move(tmp, file, StandardCopyOption.REPLACE_EXISTING);
		}
	}
}
