package fr.classroomrun.journal;

import org.jspecify.annotations.Nullable;

/** One row of the history index. {@code bestSplit} is a {@code Split} name or null. */
public record RunSummary(int run, String startedAt, long durationMs, int deaths, @Nullable String bestSplit,
		boolean finished) {
}
