package app.lockbook.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class WorkspaceAttachmentQueueTest {
    private fun attachment(id: String) = WorkspaceAttachment(id, "session", "document", "/tmp/$id", "$id.jpg", true)

    @Test fun importsAreAcknowledgedInOrder() {
        val model = WorkspaceViewModel()
        val first = attachment("first")
        val second = attachment("second")
        model.enqueueAttachment(first)
        model.enqueueAttachment(second)

        assertEquals(first, model.nextAttachment())
        assertTrue(model.markAttachmentInFlight(first.id))
        assertNull(model.nextAttachment())
        assertNull(model.completeAttachment(second.id))
        assertEquals(first, model.completeAttachment(first.id))
        assertEquals(second, model.nextAttachment())
        assertFalse(model.markAttachmentInFlight(first.id))
        assertTrue(model.markAttachmentInFlight(second.id))
        assertEquals(second, model.completeAttachment(second.id))
        assertNull(model.nextAttachment())
    }

    @Test fun workspaceReplacementAbandonsOnlyTheSubmittedHead() {
        val model = WorkspaceViewModel()
        val first = attachment("first")
        val second = attachment("second")
        model.enqueueAttachment(first)
        model.enqueueAttachment(second)

        assertNull(model.abandonInFlightAttachment())
        assertTrue(model.markAttachmentInFlight(first.id))
        assertEquals(first, model.abandonInFlightAttachment())
        assertEquals(second, model.nextAttachment())
        assertNull(model.completeAttachment(first.id))
    }
}
