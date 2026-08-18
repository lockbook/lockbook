package app.lockbook.model

import net.lockbook.File
import net.lockbook.File.FileType
import java.util.Calendar
import java.util.Locale
import java.util.TimeZone

enum class RecentPeriod {
    Today,
    Yesterday,
    PreviousSevenDays,
    Older,
}

data class RecentFileGroup(
    val period: RecentPeriod,
    val files: List<File>,
)

internal fun recentDocuments(files: Collection<File>): List<File> =
    files
        .asSequence()
        .filter { it.type == FileType.Document }
        .sortedWith(
            compareByDescending<File> { it.lastModified }
                .thenBy { it.name.lowercase(Locale.ROOT) }
                .thenBy { it.id },
        ).toList()

internal fun groupRecentDocuments(
    files: Collection<File>,
    nowMillis: Long = System.currentTimeMillis(),
    timeZone: TimeZone = TimeZone.getDefault(),
): List<RecentFileGroup> = groupFilesByRecentPeriod(recentDocuments(files), nowMillis, timeZone)

internal fun groupFilesByRecentPeriod(
    files: Collection<File>,
    nowMillis: Long = System.currentTimeMillis(),
    timeZone: TimeZone = TimeZone.getDefault(),
): List<RecentFileGroup> {
    val startOfToday =
        Calendar
            .getInstance(timeZone)
            .apply {
                timeInMillis = nowMillis
                set(Calendar.HOUR_OF_DAY, 0)
                set(Calendar.MINUTE, 0)
                set(Calendar.SECOND, 0)
                set(Calendar.MILLISECOND, 0)
            }.timeInMillis
    val startOfYesterday =
        Calendar
            .getInstance(timeZone)
            .apply {
                timeInMillis = startOfToday
                add(Calendar.DAY_OF_YEAR, -1)
            }.timeInMillis
    val startOfPreviousSevenDays =
        Calendar
            .getInstance(timeZone)
            .apply {
                timeInMillis = startOfToday
                add(Calendar.DAY_OF_YEAR, -7)
            }.timeInMillis

    return files
        .sortedWith(
            compareByDescending<File> { it.lastModified }
                .thenBy { it.name.lowercase(Locale.ROOT) }
                .thenBy { it.id },
        ).groupBy { file ->
            when {
                file.lastModified >= startOfToday -> RecentPeriod.Today
                file.lastModified >= startOfYesterday -> RecentPeriod.Yesterday
                file.lastModified >= startOfPreviousSevenDays -> RecentPeriod.PreviousSevenDays
                else -> RecentPeriod.Older
            }
        }.let { grouped ->
            RecentPeriod.entries.mapNotNull { period ->
                grouped[period]?.let { periodFiles -> RecentFileGroup(period, periodFiles) }
            }
        }
}

internal fun recentFileParentPath(
    file: File,
    filesById: Map<String, File>,
): String {
    val ancestors = mutableListOf<String>()
    val visitedIds = mutableSetOf(file.id)
    var current = file

    while (true) {
        val parent = filesById[current.parent] ?: break
        if (!visitedIds.add(parent.id) || parent.isRoot) {
            break
        }
        ancestors += parent.name
        current = parent
    }

    return ancestors.asReversed().takeIf { it.isNotEmpty() }?.joinToString(" › ") ?: "/"
}
