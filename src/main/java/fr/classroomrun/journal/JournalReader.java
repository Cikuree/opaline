package fr.classroomrun.journal;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import fr.classroomrun.net.Split;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;

/** Reads a journal. Lines that are not a JSON object with an {@code e} field (e.g. cut by a crash) are skipped. */
public final class JournalReader {
	/** What the history needs from one journal. {@code deaths} are the raw death lines. */
	public record Parsed(RunSummary summary, List<JsonObject> deaths) {
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
		Accumulator acc = new Accumulator(run);
		for (String line : lines) {
			acc.add(line);
		}
		return acc.snapshot();
	}

	/** Incremental form of {@link #parse}: feed complete lines as they appear, take an immutable snapshot at any time. */
	public static final class Accumulator {
		private static final Logger LOG = LoggerFactory.getLogger("classroomrun");
		private static boolean warned;

		private final int run;
		private String startedAt;
		private long goT = -1;
		private long lastT;
		private boolean finished;
		private int bestSplit = -1;
		private final List<JsonObject> deaths = new ArrayList<>();

		public Accumulator(int run) {
			this.run = run;
		}

		/** A line that is valid JSON but not what we expect is skipped (one warning per process), never fatal. */
		public void add(String line) {
			try {
				addLine(line);
			} catch (RuntimeException e) {
				if (!warned) {
					warned = true;
					LOG.warn("Unexpected journal line ignored in run #{} ({}); further ones are silent", this.run, e.toString());
				}
			}
		}

		private void addLine(String line) {
			JsonObject o = parseLine(line);
			if (o == null) {
				return;
			}
			String type = o.get("e").getAsString();
			if (this.startedAt == null && o.has("at")) {
				this.startedAt = o.get("at").getAsString();
			}
			if (o.has("t") && o.get("t").isJsonPrimitive() && o.get("t").getAsJsonPrimitive().isNumber()) {
				this.lastT = o.get("t").getAsLong();
			}
			switch (type) {
				case JournalEvents.START -> {
					if (o.has("at")) {
						this.startedAt = o.get("at").getAsString();
					}
				}
				case JournalEvents.GO -> {
					if (this.goT < 0) {
						this.goT = this.lastT;
					}
				}
				case JournalEvents.SPLIT -> this.bestSplit = Math.max(this.bestSplit, splitOrdinal(o));
				case JournalEvents.END -> this.finished |= o.has("reason") && "dragon".equals(o.get("reason").getAsString());
				case JournalEvents.DEATH -> {
					this.deaths.add(o);
				}
				default -> {
				}
			}
		}

		public Parsed snapshot() {
			long duration = this.goT >= 0 ? Math.max(0, this.lastT - this.goT) : 0;
			String best = this.bestSplit >= 0 ? Split.values()[this.bestSplit].name() : null;
			return new Parsed(new RunSummary(this.run, this.startedAt == null ? "" : this.startedAt, duration,
					this.deaths.size(), best, this.finished), List.copyOf(this.deaths));
		}
	}

	private static int splitOrdinal(JsonObject o) {
		try {
			return o.has("split") ? Split.valueOf(o.get("split").getAsString()).ordinal() : -1;
		} catch (IllegalArgumentException e) {
			return -1;
		}
	}
}
