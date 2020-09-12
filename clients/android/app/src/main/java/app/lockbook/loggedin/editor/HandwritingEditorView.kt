package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.PointF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View


class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    View(context, attributeSet) {
    val color = Color()
    private val lastPoint = PointF()
    private val point = PointF()
    private var scaleFactor = 1f
    val svgObject = SVG.svg { }
    private val scaleGestureDetector =
        ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScale(detector: ScaleGestureDetector): Boolean {
                scaleFactor *= detector.scaleFactor
                scaleFactor = 0.1f.coerceAtLeast(scaleFactor.coerceAtMost(5.0f))

                invalidate()
                return true
            }
        })

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            for (point in 0..event.pointerCount) {
                if (event.getToolType(point) == MotionEvent.TOOL_TYPE_STYLUS ||
                    event.getToolType(point) == MotionEvent.TOOL_TYPE_ERASER
                ) {
                    return handleStylusEvent(event)
                }

                if (event.getToolType(point) == MotionEvent.TOOL_TYPE_FINGER) {
                    return handleFingerEvent(event)
                }
            }
        } else {
            return super.onTouchEvent(event)
        }

        return false
    }

    private fun handleStylusEvent(event: MotionEvent): Boolean {
        when (event.action) {
            MotionEvent.ACTION_DOWN -> {
                lastPoint.set(event.x, event.y)
                point.set(event.x, event.y)
            }
            MotionEvent.ACTION_MOVE -> {
                svgObject.line {
                    stroke = "#FFFFFF"
                    x1 = point.x.toString()
                    y1 = point.y.toString()
                    x2 = event.x.toString()
                    y2 = event.y.toString()
                }
                lastPoint.set(point)
                point.set(event.x, event.y)
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

    override fun onDraw(canvas: Canvas?) {
        com.caverock.androidsvg.SVG.getFromString(svgObject.toString()).renderToCanvas(canvas)
        super.onDraw(canvas)
    }
}