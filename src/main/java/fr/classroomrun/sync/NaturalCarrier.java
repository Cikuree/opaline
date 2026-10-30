package fr.classroomrun.sync;

import java.util.Collection;
import java.util.UUID;

/**
 * Picks the single "carrier" player whose vanilla hunger and natural regeneration run for the whole team (pure logic).
 *
 * <p>Why: with the min/max sharing of {@link SharedVitals}, N independent {@code FoodData} would each lose food and
 * regenerate on their own timers, so the team would starve and heal about N times faster than a solo player. The
 * other members have their {@code FoodData.tick} and peaceful regeneration skipped (mixins) and the exhaustion they
 * cause is added to the carrier, so the team consumes food and heals exactly like one player doing everything.
 */
public final class NaturalCarrier {
	private NaturalCarrier() {
	}

	/**
	 * @param team    UUIDs of the synced members
	 * @param current the current carrier, may be {@code null} or no longer in the team
	 * @return {@code current} while it is still a member, otherwise the smallest UUID of the team ({@code null} if empty)
	 */
	public static UUID select(Collection<UUID> team, UUID current) {
		if (current != null && team.contains(current)) {
			return current;
		}
		return team.stream().min(UUID::compareTo).orElse(null);
	}
}
