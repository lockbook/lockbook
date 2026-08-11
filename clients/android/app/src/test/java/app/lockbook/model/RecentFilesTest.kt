package app.lockbook.model

import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.Calendar
import java.util.TimeZone

class RecentFilesTest {
    @Test
    fun `recent documents are flattened newest first without folders or links`() {
        val files =
            listOf(
                file("folder", "Folder", File.FileType.Folder, 500),
                file("old", "old.md", File.FileType.Document, 100),
                file("link", "shortcut", File.FileType.Link, 600),
                file("new", "new.md", File.FileType.Document, 300),
                file("middle", "middle.md", File.FileType.Document, 200),
            )

        assertEquals(
            listOf("new.md", "middle.md", "old.md"),
            recentDocuments(files).map { it.name },
        )
    }

    @Test
    fun `recent file path includes ancestor folders`() {
        val root = file("root", "root", File.FileType.Folder, 0).apply { parent = id }
        val projects = file("projects", "Projects", File.FileType.Folder, 0).apply { parent = root.id }
        val notes = file("notes", "Notes", File.FileType.Folder, 0).apply { parent = projects.id }
        val document = file("document", "plan.md", File.FileType.Document, 0).apply { parent = notes.id }
        val filesById = listOf(root, projects, notes, document).associateBy { it.id }

        assertEquals("Projects › Notes", recentFileParentPath(document, filesById))
    }

    @Test
    fun `recent documents are grouped into requested calendar periods`() {
        val timeZone = TimeZone.getTimeZone("UTC")
        val todayStart =
            Calendar
                .getInstance(timeZone)
                .apply {
                    set(2026, Calendar.JULY, 30, 0, 0, 0)
                    set(Calendar.MILLISECOND, 0)
                }.timeInMillis
        val files =
            listOf(
                file("today", "today.md", File.FileType.Document, todayStart + hours(2)),
                file("yesterday", "yesterday.md", File.FileType.Document, todayStart - hours(2)),
                file("week", "week.md", File.FileType.Document, todayStart - hours(72)),
                file("older", "older.md", File.FileType.Document, todayStart - hours(8 * 24)),
            )

        val groups =
            groupRecentDocuments(
                files = files,
                nowMillis = todayStart + hours(12),
                timeZone = timeZone,
            )

        assertEquals(RecentPeriod.entries, groups.map { it.period })
        assertEquals(
            listOf("today.md", "yesterday.md", "week.md", "older.md"),
            groups.flatMap { it.files }.map { it.name },
        )
    }

    @Test
    fun `generic date grouping preserves shared folders and links`() {
        val files =
            listOf(
                file("folder", "Folder", File.FileType.Folder, 300),
                file("link", "Link", File.FileType.Link, 200),
                file("document", "Document", File.FileType.Document, 100),
            )

        val grouped = groupFilesByRecentPeriod(files, nowMillis = 1_000)

        assertEquals(listOf("Folder", "Link", "Document"), grouped.flatMap { it.files }.map { it.name })
    }

    private fun hours(value: Int): Long = value * 60 * 60 * 1000L

    private fun file(
        id: String,
        name: String,
        type: File.FileType,
        modified: Long,
    ): File =
        File().apply {
            this.id = id
            parent = "root"
            this.name = name
            this.type = type
            lastModified = modified
            lastModifiedBy = ""
            shares = emptyArray()
        }
}
