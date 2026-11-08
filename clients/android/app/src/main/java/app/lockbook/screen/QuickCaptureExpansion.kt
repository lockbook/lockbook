package app.lockbook.screen

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.animation.ValueAnimator
import android.util.TypedValue
import android.view.ViewGroup
import android.widget.EditText
import android.widget.FrameLayout
import androidx.interpolator.view.animation.FastOutSlowInInterpolator
import com.google.android.material.card.MaterialCardView
import com.google.android.material.textview.MaterialTextView

/** Expands a presentation copy, keeping the original input connected to the keyboard until handoff. */
internal class QuickCaptureExpansion(
    private val root: ViewGroup,
    private val source: MaterialCardView,
    private val input: EditText,
) {
    private val surface = MaterialCardView(root.context)
    private val text = MaterialTextView(root.context)
    private var animator: ValueAnimator? = null
    private var cancelled = false

    init {
        surface.setCardBackgroundColor(source.cardBackgroundColor)
        surface.cardElevation = 0f
        surface.strokeWidth = 0
        text.setTextSize(TypedValue.COMPLEX_UNIT_PX, input.textSize)
        text.typeface = input.typeface
        text.setTextColor(input.currentTextColor)
        text.text = input.text
        surface.addView(text, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
    }

    fun updateText(value: String) {
        text.text = value
    }

    fun start(onExpanded: () -> Unit) {
        val rootLocation = IntArray(2).also(root::getLocationInWindow)
        val startLocation = IntArray(2).also(source::getLocationInWindow)
        val inputLocation = IntArray(2).also(input::getLocationInWindow)
        val startX = startLocation[0] - rootLocation[0]
        val startY = startLocation[1] - rootLocation[1]
        val startTextX = inputLocation[0] - startLocation[0] + input.totalPaddingLeft
        val startTextY = inputLocation[1] - startLocation[1] + input.totalPaddingTop
        val density = root.resources.displayMetrics.density
        root.overlay.add(surface)
        animator =
            ValueAnimator.ofFloat(0f, 1f).apply {
                duration = 320
                interpolator = FastOutSlowInInterpolator()
                addUpdateListener { animation ->
                    val progress = animation.animatedValue as Float

                    fun interpolate(
                        from: Int,
                        to: Int,
                    ): Int = (from + (to - from) * progress).toInt()
                    val left = interpolate(startX, root.paddingLeft)
                    val top = interpolate(startY, root.paddingTop)
                    val width = interpolate(source.width, root.width - root.paddingLeft - root.paddingRight)
                    val height = interpolate(source.height, root.height - root.paddingTop - root.paddingBottom)
                    surface.radius = source.radius * (1 - progress)
                    val horizontal = interpolate(startTextX, (32 * density).toInt())
                    text.setPadding(horizontal, interpolate(startTextY, (32 * density).toInt()), horizontal, 0)
                    surface.measure(
                        android.view.View.MeasureSpec
                            .makeMeasureSpec(width, android.view.View.MeasureSpec.EXACTLY),
                        android.view.View.MeasureSpec
                            .makeMeasureSpec(height, android.view.View.MeasureSpec.EXACTLY),
                    )
                    surface.layout(left, top, left + width, top + height)
                }
                addListener(
                    object : AnimatorListenerAdapter() {
                        override fun onAnimationEnd(animation: Animator) {
                            if (!cancelled) onExpanded()
                        }
                    },
                )
                start()
            }
    }

    fun finish(onFinished: () -> Unit) {
        surface
            .animate()
            .alpha(0f)
            .setDuration(160)
            .withEndAction {
                root.overlay.remove(surface)
                onFinished()
            }.start()
    }

    fun cancel() {
        cancelled = true
        animator?.cancel()
        surface.animate().cancel()
        root.overlay.remove(surface)
    }
}
