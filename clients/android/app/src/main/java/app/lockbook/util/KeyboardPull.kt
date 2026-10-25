package app.lockbook.util

import android.content.Context
import android.view.MotionEvent
import android.view.View
import android.view.inputmethod.InputMethodManager
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/** A page drag that moves down into the soft keyboard puts it away, as iOS does. */
class KeyboardPull(
    private val view: View,
) {
    private var fingerY = 0f

    /** A selection handle is being dragged; that drag never pulls. */
    var selecting = false

    fun touch(event: MotionEvent) {
        val dy = event.rawY - fingerY
        fingerY = event.rawY
        if (event.actionMasked == MotionEvent.ACTION_MOVE && dy > 0 && !selecting && fingerY >= keyboardTop()) {
            (view.context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
                .hideSoftInputFromWindow(view.windowToken, 0)
        }
    }

    /** The keyboard's top edge in screen coordinates, or infinity while it is down. */
    private fun keyboardTop(): Float {
        val ime = ViewCompat.getRootWindowInsets(view)?.getInsets(WindowInsetsCompat.Type.ime())?.bottom ?: 0
        if (ime == 0) return Float.POSITIVE_INFINITY
        val root = view.rootView
        val origin = IntArray(2).also(root::getLocationOnScreen)
        return (origin[1] + root.height - ime).toFloat()
    }
}
