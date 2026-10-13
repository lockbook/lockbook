package app.lockbook.model

import app.lockbook.util.StagedAttachment
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class WorkspaceViewModelTest {
    @Test
    fun attachmentsAreRemovedInQueueOrder() {
        val model = WorkspaceViewModel()
        val first = StagedAttachment("/tmp/first", "first.png")
        val second = StagedAttachment("/tmp/second", "second.png")

        model.enqueueAttachment(first)
        model.enqueueAttachment(second)

        assertEquals(first, model.nextAttachment())
        assertEquals(first, model.removeNextAttachment())
        assertEquals(second, model.nextAttachment())
        assertEquals(second, model.removeNextAttachment())
        assertNull(model.nextAttachment())
    }

    @Test
    fun removingFromAnEmptyQueueReturnsNull() {
        val model = WorkspaceViewModel()

        assertNull(model.removeNextAttachment())
        assertNull(model.nextAttachment())
    }
}
