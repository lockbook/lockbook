package app.lockbook.screen

import android.view.ContextThemeWrapper
import android.view.View
import androidx.recyclerview.widget.RecyclerView
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.lockbook.R
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
class RecentsItemAnimatorTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    private fun holder(): RecyclerView.ViewHolder {
        val context = ContextThemeWrapper(instrumentation.targetContext, R.style.AppTheme)
        return object : RecyclerView.ViewHolder(View(context)) {}
    }

    @Test
    fun cancellingPendingAndRunningTransitionsRestoresRecycledViews() {
        instrumentation.runOnMainSync {
            val row = holder()
            val animator = RecentsItemAnimator(row.itemView.context)
            animator.animateAdd(row)
            animator.endAnimations()
            assertFalse(animator.isRunning)
            assertEquals(1f, row.itemView.alpha)

            animator.animateMove(row, 0, 300, 0, 0)
            animator.runPendingAnimations()
            animator.endAnimations()
            assertFalse(animator.isRunning)
            assertEquals(0f, row.itemView.translationY)
            assertEquals(1f, row.itemView.alpha)
        }
    }

    @Test
    fun repeatedFilterChangesPreserveVisualPosition() {
        instrumentation.runOnMainSync {
            val row = holder()
            val animator = RecentsItemAnimator(row.itemView.context)
            animator.animateMove(row, 0, 300, 0, 100)
            animator.runPendingAnimations()
            animator.animateMove(row, 0, 100, 0, 200)
            assertEquals(100f, row.itemView.translationY)
            animator.runPendingAnimations()
            animator.endAnimations()
            assertEquals(0f, row.itemView.translationY)
            assertFalse(animator.isRunning)
        }
    }

    @Test
    fun reversingAFadePreservesCurrentOpacity() {
        instrumentation.runOnMainSync {
            val row = holder()
            val animator = RecentsItemAnimator(row.itemView.context)
            animator.animateAdd(row)
            animator.runPendingAnimations()
            row.itemView.alpha = 0.4f
            animator.animateRemove(row)
            assertEquals(0.4f, row.itemView.alpha)
            animator.runPendingAnimations()
            animator.animateAdd(row)
            assertEquals(0.4f, row.itemView.alpha)
            animator.endAnimations()
            assertEquals(1f, row.itemView.alpha)
        }
    }

    @Test
    fun cancellingOneRowDoesNotFinishOtherRows() {
        instrumentation.runOnMainSync {
            val first = holder()
            val second = holder()
            val animator = RecentsItemAnimator(first.itemView.context)
            animator.animateAdd(first)
            animator.animateRemove(second)
            animator.runPendingAnimations()
            animator.endAnimation(first)
            assertTrue(animator.isRunning)
            animator.endAnimation(second)
            assertFalse(animator.isRunning)
            assertEquals(1f, second.itemView.alpha)
        }
    }

    @Test
    fun springsFinishAndReleaseEveryRow() {
        val finished = CountDownLatch(1)
        lateinit var animator: RecentsItemAnimator
        lateinit var moving: RecyclerView.ViewHolder
        lateinit var entering: RecyclerView.ViewHolder
        instrumentation.runOnMainSync {
            moving = holder()
            entering = holder()
            animator = RecentsItemAnimator(moving.itemView.context)
            animator.animateMove(moving, 20, 300, 0, 0)
            animator.animateAdd(entering)
            animator.runPendingAnimations()
            animator.isRunning { finished.countDown() }
        }
        assertTrue("Springs should settle and notify RecyclerView", finished.await(5, TimeUnit.SECONDS))
        instrumentation.runOnMainSync {
            assertFalse(animator.isRunning)
            assertEquals(0f, moving.itemView.translationX)
            assertEquals(0f, moving.itemView.translationY)
            assertEquals(1f, entering.itemView.alpha)
        }
    }
}
