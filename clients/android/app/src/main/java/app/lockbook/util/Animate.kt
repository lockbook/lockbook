package app.lockbook.util

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.view.View

object Animate {
    fun animateVisibility(view: View, toVisibility: Int, toAlpha: Int, duration: Int) {
        val show = toVisibility == View.VISIBLE

        if (show) {
            view.alpha = 0f
            view.background.alpha = toAlpha
        }

        view.visibility = View.VISIBLE
        view.animate()
            .setDuration(duration.toLong())
            .alpha(if (show) 1f else 0f)
            .setListener(object : AnimatorListenerAdapter() {
                override fun onAnimationEnd(animation: Animator) {
                    view.visibility = toVisibility

                    if (!show) { // may need to file a bug report to android, without this visual glitches will occur
                        view.background.alpha = 255
                    }
                }
            })
    }
}
