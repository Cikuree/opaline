package fr.classroomrun.sync;

import java.util.List;

/**
 * Pure team-vitals logic (no Minecraft dependency): health, absorption, food level and saturation are shared by the
 * whole team. State = the common value ("baseline") every player had at the end of the previous tick.
 *
 * <p>Rule applied independently to each of the four values, once per server tick, over all synced players:
 * <ol>
 *   <li>if at least one player is below the baseline (damage, hunger), the shared value is the <b>minimum</b> of the
 *       players' values. Two players hit in the same tick therefore do not stack on the others: a player hit for 3
 *       and a player hit for 2 give a shared loss of 3, not 5 (shared life = min of the lives after the tick);</li>
 *   <li>otherwise, if at least one player is above the baseline (heal, regeneration, food), the shared value is the
 *       <b>maximum</b>: N players regenerating at the same time heal the team once, not N times;</li>
 *   <li>otherwise the baseline is kept.</li>
 * </ol>
 * Hunger and natural regeneration are not multiplied by the team size: only the {@link NaturalCarrier} player runs
 * them, the others get its values through this sharing. Exact hunger rule: every member action exhaustion (the
 * carrier's own included) is divided by the number of synced members and charged to the carrier
 * ({@link NaturalCarrier#share}), so a team of N doing the same thing starves like one solo player. Players who just became members (creative or spectator to
 * survival) do not take part in the merge of that tick, they receive the common value instead.
 * A loss and a gain on the same value in the same tick: the loss wins (the gain is lost). The shared value is applied
 * with silent setters (no hurt event), so the propagation never re-triggers the synchronisation.
 */
public final class SharedVitals {
	/** One player's values (health and absorption in half-hearts, food in half-drumsticks). */
	public record Vitals(float health, float absorption, float food, float saturation) {
	}

	private static final float EPSILON = 1.0E-3F;

	private Vitals baseline;

	/** Forces the common value (new run, player joining, first tick). */
	public void reset(Vitals value) {
		this.baseline = value;
	}

	public void clear() {
		this.baseline = null;
	}

	/** The common value, or {@code null} when nothing is known yet. */
	public Vitals baseline() {
		return this.baseline;
	}

	/**
	 * Computes the new common value from the players' current values and stores it as baseline.
	 *
	 * @param players values of the synced players; must not be empty
	 */
	public Vitals resolve(List<Vitals> players) {
		if (players.isEmpty()) {
			throw new IllegalArgumentException("no players");
		}
		Vitals base = this.baseline != null ? this.baseline : players.getFirst();
		float health = merge(base.health(), players.stream().mapToDouble(Vitals::health).toArray());
		float absorption = merge(base.absorption(), players.stream().mapToDouble(Vitals::absorption).toArray());
		float food = merge(base.food(), players.stream().mapToDouble(Vitals::food).toArray());
		float saturation = merge(base.saturation(), players.stream().mapToDouble(Vitals::saturation).toArray());
		this.baseline = new Vitals(health, absorption, food, saturation);
		return this.baseline;
	}

	/** Rule described in the class Javadoc, for one value. */
	static float merge(float baseline, double[] values) {
		double min = Double.MAX_VALUE;
		double max = -Double.MAX_VALUE;
		for (double v : values) {
			min = Math.min(min, v);
			max = Math.max(max, v);
		}
		if (min < baseline - EPSILON) {
			return (float) min;
		}
		if (max > baseline + EPSILON) {
			return (float) max;
		}
		return baseline;
	}
}
