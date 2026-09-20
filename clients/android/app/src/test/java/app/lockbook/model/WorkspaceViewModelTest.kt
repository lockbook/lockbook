package app.lockbook.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class WorkspaceViewModelTest {
    @Test
    fun attachmentsAreFinishedInQueueOrder() {
        val model = WorkspaceViewModel()
        val first = WorkspaceAttachment("1", "doc-1", "/tmp/first", "first.png", true)
        val second = WorkspaceAttachment("2", "doc-2", "/tmp/second", "second.pdf", false)

        model.enqueueAttachment(first)
        model.enqueueAttachment(second)

        assertEquals(first, model.nextAttachment())
        model.markAttachmentInFlight(first.id)
        assertNull(model.nextAttachment())
        model.completeAttachment(first.id)
        assertEquals(second, model.nextAttachment())
        model.markAttachmentInFlight(second.id)
        model.completeAttachment(second.id)
        assertNull(model.nextAttachment())
    }

    @Test
    fun onlyMatchingCompletionAdvancesQueue() {
        val model = WorkspaceViewModel()
        val attachment = WorkspaceAttachment("1", "doc-1", "/tmp/first", "first.png", true)
        model.enqueueAttachment(attachment)
        model.markAttachmentInFlight(attachment.id)

        assertNull(model.completeAttachment("another-request"))
        assertNull(model.nextAttachment())
        assertEquals(attachment, model.completeAttachment(attachment.id))
    }
}
