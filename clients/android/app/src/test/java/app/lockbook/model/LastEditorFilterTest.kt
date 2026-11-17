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
