package fr.classroomrun.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonParseException;
import org.slf4j.Logger;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/** {@code config/classroomrun/config.json}. Written with the defaults when missing. */
public final class ModConfig {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	/** Port of the read-only JSON endpoint; 0 disables it. */
	public int httpPort = 25580;
	/** Delay between the collective death and the reset, so clients can show the death recap. 0 = same tick. */
	public int deathResetDelayTicks = 60;

	public static ModConfig load(Path file, Logger log) {
		ModConfig config = new ModConfig();
		try {
			if (Files.exists(file)) {
				ModConfig read = GSON.fromJson(Files.readString(file, StandardCharsets.UTF_8), ModConfig.class);
				if (read != null) {
					config = read;
				}
			} else {
				Files.createDirectories(file.toAbsolutePath().getParent());
				Files.writeString(file, GSON.toJson(config), StandardCharsets.UTF_8);
			}
		} catch (IOException | JsonParseException e) {
			log.warn("Cannot read {} ({}), using defaults", file, e.getMessage());
		}
		return config;
	}
}
