package app.lockbook.model

import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RecentsFilterTest {
    @Test
    fun `filter is hidden without another account and never hides files`() {
        val state = RecentsFilterState(listOf("me"), emptySet())
        assertFalse(state.isVisible)
        assertTrue(state.includesEditor("me"))
    }

    @Test
    fun `all and none are different selections`() {
        val users = listOf("me", "friend")
        assertTrue(RecentsFilterState(users).includesEditor("friend"))
        assertFalse(RecentsFilterState(users, emptySet()).includesEditor("friend"))
    }

    @Test
    fun `filter uses last editor rather than file owner`() {
        val files = listOf(file("mine", "me", 20), file("theirs", "friend", 10))
        val state = RecentsFilterState(listOf("me", "friend"), setOf("friend"))
        assertEquals(listOf("theirs"), state.filterByEditor(files).map { it.id })
    }

    @Test
    fun `welcome filter runs before recent limit and includes matching pins`() {
        val files = (1..30).map { file("$it", if (it > 15) "me" else "friend", it.toLong()) }
        val state = RecentsFilterState(listOf("me", "friend"), setOf("friend"))
        val items = workspaceDocumentItems(listOf(PinnedFile("1"), PinnedFile("30")), state.filterByEditor(files))
        assertEquals("1", items.first().file.id)
        assertEquals(13, items.size)
        assertTrue(items.all { it.file.lastModifiedBy == "friend" })
        assertEquals("15", items[1].file.id)
    }

    @Test
    fun `account username stays first and contact ranking is preserved`() {
        val state = RecentsFilterState().withContacts(listOf("zoe", "me", "amy", "zoe", ""), "me")
        assertEquals(listOf("me", "zoe", "amy"), state.usernames)
    }

    @Test
    fun `reenabling one editor preserves other editor selections`() {
        val initial = RecentsFilterState(listOf("me", "zoe", "amy"))
        val hidden = initial.withUserEnabled("zoe", false).withUserEnabled("amy", false)
        assertEquals(listOf("me"), hidden.usernames.filter(hidden::includesEditor))
        val restored = hidden.withUserEnabled("zoe", true)
        assertEquals(listOf("me", "zoe"), restored.usernames.filter(restored::includesEditor))
        assertEquals(restored, restored.withUserEnabled("zoe", true))
    }

    @Test
    fun `one editor can be enabled when all editors are deselected`() {
        val state = RecentsFilterState(listOf("me", "zoe"), emptySet()).withUserEnabled("zoe", true)
        assertFalse(state.includesEditor("me"))
        assertTrue(state.includesEditor("zoe"))
    }

    @Test
    fun `chips show only editors present in recents while retaining contact order`() {
        val state = RecentsFilterState(listOf("me", "zoe", "amy", "unused"))
        val files = listOf(file("a", "amy", 20), file("b", "zoe", 10))
        assertEquals(listOf("zoe", "amy"), state.recentUsernames(files))
        assertEquals(emptyList<String>(), state.recentUsernames(emptyList()))
    }

    @Test
    fun `deselecting an editor leaves its chip available in the full recents list`() {
        val files = listOf(file("a", "me", 20), file("b", "zoe", 10))
        val state = RecentsFilterState(listOf("me", "zoe")).withUserEnabled("zoe", false)
        assertEquals(listOf("a"), state.filterByEditor(files).map { it.id })
        assertEquals(listOf("me", "zoe"), state.recentUsernames(files))
    }

    @Test
    fun `document types combine with OR and editors combine with AND`() {
        val files =
            listOf(
                file("note", "me", 40),
                file("image", "zoe", 30).apply { name = "photo.PNG" },
                file("drawing", "zoe", 20).apply { name = "sketch.svg" },
                file("pdf", "zoe", 10).apply { name = "report.pdf" },
            )
        val all = RecentsFilterState(listOf("me", "zoe"))
        assertEquals(files, all.filterRecents(files))
        val types =
            all
                .withDocumentTypeToggled(RecentDocumentType.Notes)
                .withDocumentTypeToggled(RecentDocumentType.Images)
        assertEquals(listOf("note", "image"), types.filterRecents(files).map { it.id })
        val editors = types.withUserEnabled("me", false)
        assertEquals(listOf("image"), editors.filterRecents(files).map { it.id })
        assertEquals(listOf("me", "zoe"), editors.recentUsernames(files))
        assertEquals(
            files,
            types
                .withDocumentTypeToggled(RecentDocumentType.Notes)
                .withDocumentTypeToggled(RecentDocumentType.Images)
                .filterRecents(files),
        )
        // The welcome screen continues sharing editor selection, without the Recents-only types.
        assertEquals(listOf("image", "drawing", "pdf"), editors.filterByEditor(files).map { it.id })
    }

    @Test
    fun `document type filters work when there are no other contacts`() {
        val state = RecentsFilterState(listOf("me")).withDocumentTypeToggled(RecentDocumentType.Drawings)
        val files = listOf(file("note", "me", 20), file("sketch", "me", 10).apply { name = "sketch.svg" })
        assertEquals(listOf("sketch"), state.filterRecents(files).map { it.id })
    }

    @Test
    fun `notes include markdown and plain text and respect editor selection`() {
        val files =
            listOf(
                file("md", "me", 30),
                file("text", "zoe", 20).apply { name = "notes.TXT" },
                file("image", "zoe", 10).apply { name = "photo.png" },
            )
        val notes =
            RecentsFilterState(listOf("me", "zoe"))
                .withDocumentTypeToggled(RecentDocumentType.Notes)
        assertEquals(listOf("md", "text"), notes.filterRecents(files).map { it.id })
        assertEquals(listOf("text"), notes.withUserEnabled("me", false).filterRecents(files).map { it.id })
    }

    private fun file(
        id: String,
        editor: String,
        modified: Long,
    ): File =
        File().apply {
            this.id = id
            parent = "root"
            name = "$id.md"
            type = File.FileType.Document
            lastModified = modified
            lastModifiedBy = editor
            shares = emptyArray()
        }
}
