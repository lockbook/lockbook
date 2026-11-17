package app.lockbook.model

import net.lockbook.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LastEditorFilterTest {
    @Test
    fun `filter is hidden without another account and never hides files`() {
        val state = LastEditorFilterState(listOf("me"), emptySet())
        assertFalse(state.isVisible)
        assertTrue(state.includes("me"))
    }

    @Test
    fun `all and none are different selections`() {
        val users = listOf("me", "friend")
        assertTrue(LastEditorFilterState(users).includes("friend"))
        assertFalse(LastEditorFilterState(users, emptySet()).includes("friend"))
    }

    @Test
    fun `filter uses last editor rather than file owner`() {
        val files = listOf(file("mine", "me", 20), file("theirs", "friend", 10))
        val state = LastEditorFilterState(listOf("me", "friend"), setOf("friend"))
        assertEquals(listOf("theirs"), state.filter(files).map { it.id })
    }

    @Test
    fun `welcome filter runs before recent limit and includes matching pins`() {
        val files = (1..30).map { file("$it", if (it > 15) "me" else "friend", it.toLong()) }
        val state = LastEditorFilterState(listOf("me", "friend"), setOf("friend"))
        val items = workspaceDocumentItems(listOf(PinnedFile("1"), PinnedFile("30")), state.filter(files))
        assertEquals("1", items.first().file.id)
        assertEquals(13, items.size)
        assertTrue(items.all { it.file.lastModifiedBy == "friend" })
        assertEquals("15", items[1].file.id)
    }

    @Test
    fun `yourself stays first and contact ranking is preserved`() {
        val state = LastEditorFilterState().withContacts(listOf("zoe", "me", "amy", "zoe", ""), "me")
        assertEquals(listOf("me", "zoe", "amy"), state.usernames)
    }

    @Test
    fun `undo restores only the removed editor without undoing later filters`() {
        val initial = LastEditorFilterState(listOf("me", "zoe", "amy"))
        val hidden = initial.withUserEnabled("zoe", false).withUserEnabled("amy", false)
        assertEquals(listOf("me"), hidden.usernames.filter(hidden::includes))
        val undone = hidden.withUserEnabled("zoe", true)
        assertEquals(listOf("me", "zoe"), undone.usernames.filter(undone::includes))
        assertEquals(undone, undone.withUserEnabled("zoe", true))
    }

    @Test
    fun `modal can enable one user after deselecting everyone`() {
        val state = LastEditorFilterState(listOf("me", "zoe"), emptySet()).withUserEnabled("zoe", true)
        assertFalse(state.includes("me"))
        assertTrue(state.includes("zoe"))
    }

    @Test
    fun `chips show only editors present in recents while retaining contact order`() {
        val state = LastEditorFilterState(listOf("me", "zoe", "amy", "unused"))
        val files = listOf(file("a", "amy", 20), file("b", "zoe", 10))
        assertEquals(listOf("zoe", "amy"), state.recentUsernames(files))
        assertEquals(emptyList<String>(), state.recentUsernames(emptyList()))
    }

    @Test
    fun `deselecting an editor leaves its chip available in the full recents list`() {
        val files = listOf(file("a", "me", 20), file("b", "zoe", 10))
        val state = LastEditorFilterState(listOf("me", "zoe")).withUserEnabled("zoe", false)
        assertEquals(listOf("a"), state.filter(files).map { it.id })
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
        val all = LastEditorFilterState(listOf("me", "zoe"))
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
        assertEquals(listOf("image", "drawing", "pdf"), editors.filter(files).map { it.id })
    }

    @Test
    fun `document type filters work when there are no other contacts`() {
        val state = LastEditorFilterState(listOf("me")).withDocumentTypeToggled(RecentDocumentType.Drawings)
        val files = listOf(file("note", "me", 20), file("sketch", "me", 10).apply { name = "sketch.svg" })
        assertEquals(listOf("sketch"), state.filterRecents(files).map { it.id })
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
