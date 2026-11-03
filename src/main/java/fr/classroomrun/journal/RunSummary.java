package fr.classroomrun.journal;

import java.util.List;

import org.jspecify.annotations.Nullable;

/** One row of the history index. {@code bestSplit} is a {@code Split} name or null; {@code victims}: name of each death. */
public record RunSummary(int run, String startedAt, long durationMs, int deaths, @Nullable String bestSplit,
		boolean finished, List<String> victims) {
}
