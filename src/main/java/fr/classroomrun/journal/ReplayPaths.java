package fr.classroomrun.journal;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.InvalidPathException;
import java.nio.file.Path;
import java.util.Optional;
import java.util.Set;

/** Whitelist + canonicalisation for the replay download route. Pure IO, no Minecraft dependency. */
public final class ReplayPaths {
	private ReplayPaths() {
	}

	/**
	 * @param whitelist replay paths found in the journals (opaque relative strings, '/' separated)
	 * @param root      directory that must contain the file once canonicalised
	 * @return the file to serve, empty (=> 404) for anything not whitelisted, outside {@code root} or not a regular file
	 */
	public static Optional<Path> resolve(Set<String> whitelist, Path root, String raw) {
		if (raw == null || raw.isEmpty() || !whitelist.contains(raw)) {
			return Optional.empty();
		}
		try {
			Path canonicalRoot = root.toRealPath();
			Path file = canonicalRoot.resolve(raw).toRealPath();
			if (file.startsWith(canonicalRoot) && Files.isRegularFile(file)) {
				return Optional.of(file);
			}
		} catch (IOException | InvalidPathException e) {
			// missing file or invalid path: not served
		}
		return Optional.empty();
	}
}
