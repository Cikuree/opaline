package fr.classroomrun.client.history;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import org.jspecify.annotations.Nullable;

import java.time.Instant;
import java.time.ZoneId;
import java.time.format.DateTimeFormatter;
import java.time.format.DateTimeParseException;
import java.util.ArrayList;
import java.util.List;

/**
 * Parsed history JSON (schemas in CONTRACT.md) and display helpers. Pure, no Minecraft dependency. Parsing is
 * tolerant: a missing or mistyped field gets a neutral default, invalid JSON gives an empty result.
 */
public final class HistoryModel {
	public record RunRow(int run, String startedAt, long durationMs, int deaths, @Nullable String bestSplit, boolean finished) {
	}

	public record Sample(long relMs, float hp) {
	}

	public record Series(String name, List<Sample> samples) {
	}

	public record Death(String at, String name, String cause, String message, @Nullable String attacker, String fatalBy,
			int x, int y, int z, String dimension, List<Series> health) {
	}

	public record RunDetail(int run, String startedAt, List<Death> deaths) {
	}

	private static final DateTimeFormatter WHEN = DateTimeFormatter.ofPattern("dd/MM HH:mm:ss");

	private HistoryModel() {
	}

	public static List<RunRow> parseIndex(String json) {
		List<RunRow> rows = new ArrayList<>();
		JsonObject root = root(json);
		if (root == null || !root.has("runs") || !root.get("runs").isJsonArray()) {
			return rows;
		}
		for (JsonElement e : root.getAsJsonArray("runs")) {
			if (e.isJsonObject()) {
				JsonObject o = e.getAsJsonObject();
				rows.add(new RunRow(integer(o, "run"), string(o, "startedAt"), (long) number(o, "durationMs"), integer(o, "deaths"),
						nullableString(o, "bestSplit"), o.has("finished") && o.get("finished").isJsonPrimitive()
								&& o.get("finished").getAsBoolean()));
			}
		}
		return rows;
	}

	public static RunDetail parseDetail(String json) {
		JsonObject root = root(json);
		if (root == null) {
			return new RunDetail(-1, "", List.of());
		}
		List<Death> deaths = new ArrayList<>();
		if (root.has("deaths") && root.get("deaths").isJsonArray()) {
			for (JsonElement e : root.getAsJsonArray("deaths")) {
				if (e.isJsonObject()) {
					deaths.add(death(e.getAsJsonObject()));
				}
			}
		}
		return new RunDetail(integer(root, "run"), string(root, "startedAt"), deaths);
	}

	private static Death death(JsonObject o) {
		int x = 0;
		int y = 0;
		int z = 0;
		if (o.has("pos") && o.get("pos").isJsonArray() && o.getAsJsonArray("pos").size() == 3) {
			JsonArray p = o.getAsJsonArray("pos");
			x = (int) element(p.get(0));
			y = (int) element(p.get(1));
			z = (int) element(p.get(2));
		}
		List<Series> health = new ArrayList<>();
		if (o.has("health") && o.get("health").isJsonArray()) {
			for (JsonElement s : o.getAsJsonArray("health")) {
				if (s.isJsonObject()) {
					health.add(new Series(string(s.getAsJsonObject(), "name"), samples(s.getAsJsonObject())));
				}
			}
		}
		return new Death(string(o, "at"), string(o, "name"), string(o, "cause"), string(o, "message"),
				nullableString(o, "attacker"), string(o, "fatalBy"), x, y, z, string(o, "dimension"), health);
	}

	private static List<Sample> samples(JsonObject series) {
		List<Sample> out = new ArrayList<>();
		if (series.has("samples") && series.get("samples").isJsonArray()) {
			for (JsonElement p : series.getAsJsonArray("samples")) {
				if (p.isJsonArray() && p.getAsJsonArray().size() == 2) {
					out.add(new Sample((long) element(p.getAsJsonArray().get(0)), (float) element(p.getAsJsonArray().get(1))));
				}
			}
		}
		return out;
	}

	private static @Nullable JsonObject root(String json) {
		try {
			JsonElement e = JsonParser.parseString(json);
			return e.isJsonObject() ? e.getAsJsonObject() : null;
		} catch (JsonParseException e) {
			return null;
		}
	}

	private static double element(JsonElement e) {
		return e.isJsonPrimitive() && e.getAsJsonPrimitive().isNumber() ? e.getAsDouble() : 0.0;
	}

	private static double number(JsonObject o, String key) {
		return o.has(key) ? element(o.get(key)) : 0.0;
	}

	private static int integer(JsonObject o, String key) {
		return (int) number(o, key);
	}

	private static String string(JsonObject o, String key) {
		String s = nullableString(o, key);
		return s == null ? "" : s;
	}

	private static @Nullable String nullableString(JsonObject o, String key) {
		return o.has(key) && o.get(key).isJsonPrimitive() ? o.get(key).getAsString() : null;
	}

	// ---------------------------------------------------------------- display

	public static String duration(long ms) {
		long s = Math.max(0, ms) / 1000;
		return String.format("%d:%02d:%02d", s / 3600, s / 60 % 60, s % 60);
	}

	/** ISO instant to local "dd/MM HH:mm:ss"; the raw text if it cannot be parsed. */
	public static String when(String iso) {
		try {
			return WHEN.format(Instant.parse(iso).atZone(ZoneId.systemDefault()));
		} catch (DateTimeParseException e) {
			return iso;
		}
	}
}
