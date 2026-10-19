package app.lockbook.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MarkdownPreviewTest {
    @Test
    fun `a cursor on trailing blank lines keeps the last paragraph in view`() {
        val text = (1..100).joinToString("\n") { "Paragraph $it" } + "\n\n"
        assertEquals("Paragraph 100", markdownExcerpt(text, text.length).focusLine)
    }

    @Test
    fun `excerpt follows a cursor deep in the note instead of always taking the beginning`() {
        val text = (1..100).joinToString("\n") { "Paragraph $it" }
        val excerpt = markdownExcerpt(text, text.indexOf("Paragraph 60") + 5)
        assertTrue(excerpt.markdown.contains("Paragraph 60"))
        assertFalse(excerpt.markdown.contains("Paragraph 1\n"))
        assertEquals("Paragraph 60", excerpt.focusLine)
    }

    @Test
    fun `missing and stale cursors are safely clamped`() {
        val text = "# Heading\n\nA note"
        assertTrue(markdownExcerpt(text, -100).markdown.contains("# Heading"))
        assertTrue(markdownExcerpt(text, Int.MAX_VALUE).markdown.contains("A note"))
        assertEquals(MarkdownExcerpt("", ""), markdownExcerpt(" \n", 0))
    }

    @Test
    fun `excerpt preserves code fence formatting around the cursor`() {
        val text = "```kotlin\n" + (1..60).joinToString("\n") { "val item$it = $it" } + "\n```"
        val excerpt = markdownExcerpt(text, text.indexOf("val item30"))
        assertTrue(excerpt.markdown.startsWith("```kotlin\n"))
        assertTrue(excerpt.markdown.endsWith("\n```"))
        assertTrue(excerpt.markdown.contains("val item30"))
    }

    @Test
    fun `long lines are bounded and retain the cursor context without splitting surrogate pairs`() {
        val text = "😀".repeat(3000) + "target" + "😀".repeat(3000)
        val excerpt = markdownExcerpt(text, text.indexOf("target"))
        assertTrue(excerpt.markdown.contains("target"))
        assertTrue(excerpt.markdown.length <= 1400)
        assertFalse(excerpt.markdown.first().isLowSurrogate())
        assertFalse(excerpt.markdown.last().isHighSurrogate())
    }

    @Test
    fun `saved cursor schema uses the active selection end and ignores invalid entries`() {
        val json = """{"markdown":{"file":{"a":{"selection":[4,29]},"b":{"selection":[0,-1]},"broken":{"selection":"bad"}}}}"""
        assertEquals(mapOf("a" to 29, "b" to 0), savedMarkdownCursors(json))
        assertEquals(emptyMap<String, Int>(), savedMarkdownCursors("{}"))
    }

    @Test
    fun `card data changes when the saved cursor changes without editing the file`() {
        val file =
            net.lockbook.File().apply {
                id = "note"
                name = "note.md"
                type = net.lockbook.File.FileType.Document
                lastModified = 1
            }
        val old = workspaceDocumentItems(emptyList(), listOf(file), mapOf("note" to 2)).single()
        val updated = workspaceDocumentItems(emptyList(), listOf(file), mapOf("note" to 30)).single()
        assertFalse(old == updated)
    }
}
