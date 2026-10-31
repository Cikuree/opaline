package fr.classroomrun.journal;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import java.time.Instant;
import java.util.List;

/**
 * Builders of the journal lines (one JSON object per line, field {@code e} = event type, {@code at} = ISO instant,
 * {@code t} = cumulative chrono in ms). Pure, no Minecraft dependency. Health values are health points (20.0 = full).
 */
public final class JournalEvents {
	public static final String START = "start";
	public static final String GO = "go";
	public static final String DAMAGE = "damage";
	public static final String HEAL = "heal";
	public static final String DEATH = "death";
	public static final String SPLIT = "split";
	public static final String END = "end";

	private JournalEvents() {
	}

	public static JsonObject event(String type, Instant at, long chronoMs) {
		JsonObject o = new JsonObject();
		o.addProperty("e", type);
		o.addProperty("at", at.toString());
		o.addProperty("t", chronoMs);
		return o;
	}

	public static JsonArray pos(int x, int y, int z) {
		JsonArray a = new JsonArray();
		a.add(x);
		a.add(y);
		a.add(z);
		return a;
	}

	public static JsonArray health(List<HealthBuffer.PlayerSamples> samples) {
		JsonArray all = new JsonArray();
		for (HealthBuffer.PlayerSamples p : samples) {
			JsonObject o = new JsonObject();
			o.addProperty("name", p.name());
			JsonArray points = new JsonArray();
			for (HealthBuffer.Point point : p.samples()) {
				JsonArray pair = new JsonArray();
				pair.add(point.relMs());
				pair.add(point.hp());
				points.add(pair);
			}
			o.add("samples", points);
			all.add(o);
		}
		return all;
	}
}
