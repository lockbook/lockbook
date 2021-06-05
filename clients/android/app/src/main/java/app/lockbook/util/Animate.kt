package app.lockbook.util

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.view.View
import timber.log.Timber

object Animate {
    fun animateVisibility(view: View, toVisibility: Int, toAlpha: Float, duration: Int) {
        Timber.e("SWITCHING: $toVisibility $toAlpha $duration")

        val show = toVisibility == View.VISIBLE
        if (show) {
            view.alpha = 0f
        }

        view.visibility = View.VISIBLE
        view.animate()
            .setDuration(duration.toLong())
            .alpha(if (show) toAlpha else 0f)
            .setListener(object : AnimatorListenerAdapter() {
                override fun onAnimationEnd(animation: Animator) {
                    view.visibility = toVisibility
                }
            })
    }
}
