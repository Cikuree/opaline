package fr.classroomrun.journal;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import fr.classroomrun.net.Split;
import org.jspecify.annotations.Nullable;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** Reads a journal. Lines that are not a JSON object with an {@code e} field (e.g. cut by a crash) are skipped. */
public final class JournalReader {
	/** What the history needs from one journal. {@code deaths} are the raw death lines. */
	public record Parsed(RunSummary summary, List<JsonObject> deaths, Set<String> replayPaths) {
	}

	private JournalReader() {
	}

	public static @Nullable JsonObject parseLine(String line) {
		if (line.isBlank()) {
			return null;
		}
		try {
			JsonElement e = JsonParser.parseString(line);
			if (e.isJsonObject() && e.getAsJsonObject().has("e") && e.getAsJsonObject().get("e").isJsonPrimitive()) {
				return e.getAsJsonObject();
			}
		} catch (JsonParseException | IllegalStateException ex) {
			// truncated or corrupted line
		}
		return null;
	}

	public static Parsed parse(int run, Iterable<String> lines) {
		String startedAt = null;
		long goT = -1;
		long lastT = 0;
		boolean finished = false;
		int bestSplit = -1;
		List<JsonObject> deaths = new ArrayList<>();
		Set<String> paths = new HashSet<>();
		for (String line : lines) {
			JsonObject o = parseLine(line);
			if (o == null) {
				continue;
			}
			String type = o.get("e").getAsString();
			if (startedAt == null && o.has("at")) {
				startedAt = o.get("at").getAsString();
			}
			if (o.has("t") && o.get("t").isJsonPrimitive() && o.get("t").getAsJsonPrimitive().isNumber()) {
				lastT = o.get("t").getAsLong();
			}
			switch (type) {
				case JournalEvents.START -> {
					if (o.has("at")) {
						startedAt = o.get("at").getAsString();
					}
				}
				case JournalEvents.GO -> {
					if (goT < 0) {
						goT = lastT;
					}
				}
				case JournalEvents.SPLIT -> bestSplit = Math.max(bestSplit, splitOrdinal(o));
				case JournalEvents.END -> finished |= o.has("reason") && "dragon".equals(o.get("reason").getAsString());
				case JournalEvents.DEATH -> {
					deaths.add(o);
					if (o.has("replay") && o.get("replay").isJsonObject() && o.getAsJsonObject("replay").has("path")) {
						paths.add(o.getAsJsonObject("replay").get("path").getAsString());
					}
				}
				default -> {
				}
			}
		}
		long duration = goT >= 0 ? Math.max(0, lastT - goT) : 0;
		String best = bestSplit >= 0 ? Split.values()[bestSplit].name() : null;
		return new Parsed(new RunSummary(run, startedAt == null ? "" : startedAt, duration, deaths.size(), best, finished),
				deaths, paths);
	}

	private static int splitOrdinal(JsonObject o) {
		try {
			return o.has("split") ? Split.valueOf(o.get("split").getAsString()).ordinal() : -1;
		} catch (IllegalArgumentException e) {
			return -1;
		}
	}
}
