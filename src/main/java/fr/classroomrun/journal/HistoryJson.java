package fr.classroomrun.journal;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;

import java.util.List;

/**
 * Builds the JSON of {@code HistoryPayload} (schemas in CONTRACT.md, section "Historique des morts"). Both builders
 * drop the oldest entries until the text fits {@code maxChars}, so the result is always valid JSON.
 */
public final class HistoryJson {
	public static final int MAX_RUNS = 200;
	private static final Gson GSON = new com.google.gson.GsonBuilder().serializeNulls().create(); // the contract has explicit nulls

	private HistoryJson() {
	}

	/** @param runsNewestFirst summaries sorted from the most recent to the oldest */
	public static String index(List<RunSummary> runsNewestFirst, int maxChars) {
		int count = Math.min(MAX_RUNS, runsNewestFirst.size());
		while (true) {
			JsonArray runs = new JsonArray();
			for (RunSummary s : runsNewestFirst.subList(0, count)) {
				JsonObject o = new JsonObject();
				o.addProperty("run", s.run());
				o.addProperty("startedAt", s.startedAt());
				o.addProperty("durationMs", s.durationMs());
				o.addProperty("deaths", s.deaths());
				o.addProperty("bestSplit", s.bestSplit()); // null => JSON null
				o.addProperty("finished", s.finished());
				runs.add(o);
			}
			JsonObject root = new JsonObject();
			root.add("runs", runs);
			String json = GSON.toJson(root);
			if (json.length() <= maxChars || count == 0) {
				return json;
			}
			count--; // the oldest ones are at the end
		}
	}

	/** @param deathLines raw journal death lines, oldest first */
	public static String detail(int run, String startedAt, List<JsonObject> deathLines, int maxChars) {
		int from = 0;
		boolean withHealth = true;
		while (true) {
			JsonArray deaths = new JsonArray();
			for (JsonObject line : deathLines.subList(from, deathLines.size())) {
				deaths.add(entry(line, withHealth));
			}
			JsonObject root = new JsonObject();
			root.addProperty("run", run);
			root.addProperty("startedAt", startedAt);
			root.add("deaths", deaths);
			String json = GSON.toJson(root);
			if (json.length() <= maxChars || from >= deathLines.size()) {
				return json;
			}
			if (deathLines.size() - from > 1) {
				from++; // drop the oldest death
			} else if (withHealth) {
				withHealth = false; // a single death that is still too big: keep it without its graph
			} else {
				from++;
			}
		}
	}

	/** A death line without the journal-only fields. */
	private static JsonObject entry(JsonObject line, boolean withHealth) {
		JsonObject o = new JsonObject();
		for (String key : new String[] {"at", "player", "name", "cause", "message", "attacker", "fatalBy", "pos",
				"dimension"}) {
			JsonElement v = line.get(key);
			o.add(key, v == null ? JsonNull.INSTANCE : v);
		}
		o.add("health", withHealth && line.has("health") ? line.get("health") : new JsonArray());
		return o;
	}
}
