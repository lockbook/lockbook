package app.lockbook.loggedin.editor

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import androidx.core.graphics.scaleMatrix


class HandwritingView(context: Context, attributeSet: AttributeSet?) : View(context, attributeSet) {
    private val paint: Paint? = null
    private val path: Path? = null
    private var scaleFactor = 1f
    private val scaleGestureDetector = ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScale(detector: ScaleGestureDetector): Boolean {
                scaleFactor *= detector.scaleFactor

                scaleFactor = 0.1f.coerceAtLeast(scaleFactor.coerceAtMost(5.0f))

                invalidate()
                return true
            }
        })

    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            if (event.getToolType(event.toolMajor.toInt()) == MotionEvent.TOOL_TYPE_STYLUS ||
                event.getToolType(event.toolMajor.toInt()) == MotionEvent.TOOL_TYPE_ERASER
            ) {
                return handleStylusEvent(event)
            }

            if (event.getToolType(event.toolMajor.toInt()) == MotionEvent.TOOL_TYPE_FINGER) {
                return handleFingerEvent(event)
            }
        } else {
            return super.onTouchEvent(event)
        }

        return false
    }

    private fun handleStylusEvent(event: MotionEvent): Boolean {
        when (event.action) {
            MotionEvent.ACTION_DOWN -> path?.moveTo(event.x, event.y)
            MotionEvent.ACTION_MOVE -> {
                path?.lineTo(event.x, event.y)
                invalidate()
            }
            MotionEvent.ACTION_UP -> {
            }
        }

        return true
    }

    private fun handleFingerEvent(event: MotionEvent): Boolean {
        scaleGestureDetector.onTouchEvent(event)

        return true
    }

    override fun onDraw(canvas: Canvas) {
        if (path != null && paint != null) {
            canvas.drawPath(path, paint)
        }
        scaleMatrix(scaleFactor, scaleFactor)

        super.onDraw(canvas)
    }


}