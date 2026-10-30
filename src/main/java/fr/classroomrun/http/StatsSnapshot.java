package fr.classroomrun.http;

import com.google.gson.Gson;
import fr.classroomrun.net.Split;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Immutable copy of the run state, built on the server thread and read by the HTTP thread. */
public record StatsSnapshot(int runNumber, long totalTimeMs, int deaths, int currentSplit, int reachedSplits,
		int bestSplit, List<String> players, boolean running) {
	private static final Gson GSON = new Gson();

	public StatsSnapshot {
		players = List.copyOf(players);
	}

	/** Body of {@code GET /stats}. Splits are given by name; the contract index is kept in {@code currentSplit}. */
	public String toJson() {
		Split[] splits = Split.values();
		List<String> reached = new ArrayList<>();
		for (Split s : splits) {
			if ((this.reachedSplits & s.bit()) != 0) {
				reached.add(s.name());
			}
		}
		Map<String, Object> json = new LinkedHashMap<>();
		json.put("runNumber", this.runNumber);
		json.put("totalTimeMs", this.totalTimeMs);
		json.put("running", this.running);
		json.put("deaths", this.deaths);
		json.put("currentSplit", this.currentSplit);
		json.put("currentSplitName", this.currentSplit < splits.length ? splits[this.currentSplit].name() : null);
		json.put("reachedSplits", reached);
		json.put("bestSplit", this.bestSplit >= 0 ? splits[this.bestSplit].name() : null);
		json.put("players", this.players);
		return GSON.toJson(json);
	}
}
