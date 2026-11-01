package app.lockbook.util

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WorkspaceClipboardTest {
    @Test
    fun `accepts svg from vector editors with XML preambles`() {
        listOf(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            " \n<svg>content</svg>",
            "\uFEFF<?xml version=\"1.0\"?>\n<!-- exported drawing -->\n<svg/>",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE svg PUBLIC \"example\" \"example.dtd\">\n<svg/>",
        ).forEach { assertTrue(it, isSvgClipboardText(it)) }
    }

    @Test
    fun `rejects ordinary text HTML and incomplete preambles`() {
        listOf(
            "",
            "some <svg> markup",
            "<html><svg/></html>",
            "<svgSomething/>",
            "<?xml version=\"1.0\" <svg/>",
            "<!-- <svg/>",
            "<!DOCTYPE svg <svg/>",
        ).forEach { assertFalse(it, isSvgClipboardText(it)) }
    }
}
