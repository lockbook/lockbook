package app.lockbook.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CreateFileViewModelTest {
    @Test
    fun fileTypesApplyOnlyTheirOwnedExtensions() {
        assertEquals("meeting.md", NewFileType.Note.completeName("meeting"))
        assertEquals("sketch.svg", NewFileType.Drawing.completeName("sketch"))
        assertEquals("Projects", NewFileType.Folder.completeName("Projects"))
        assertEquals("data.json", NewFileType.Other.completeName("data.json"))
    }

    @Test
    fun onlyFolderCreatesFolderMetadata() {
        assertFalse(NewFileType.Folder.isDocument)
        assertTrue(NewFileType.Note.isDocument)
        assertTrue(NewFileType.Drawing.isDocument)
        assertTrue(NewFileType.Other.isDocument)
    }
}
