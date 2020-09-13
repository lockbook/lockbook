package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PointF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import app.lockbook.utils.Path


class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    View(context, attributeSet) {
    private val paint = Paint()
    var path = Path()
    private var scaleFactor = 1f
    private val canvasPath = android.graphics.Path()
    private val scaleGestureDetector =
        ScaleGestureDetector(context, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScale(detector: ScaleGestureDetector): Boolean {
                scaleFactor *= detector.scaleFactor
                scaleFactor = 0.1f.coerceAtLeast(scaleFactor.coerceAtMost(5.0f))

                invalidate()
                return true
            }
        })

    init {
        paint.isAntiAlias = true
        paint.color = Color.WHITE
        paint.style = Paint.Style.STROKE
        paint.strokeJoin = Paint.Join.MITER
        paint.strokeWidth = 0f
    }

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
                path.points.add(PointF(Float.MIN_VALUE, Float.MIN_VALUE))
                path.points.add(PointF(event.x, event.y))
            }
            MotionEvent.ACTION_MOVE -> {
                path.points.add(PointF(event.x, event.y))
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
        for (index in 0 until path.points.size) {
            if (index != 0 && path.points[index - 1].x != Float.MIN_VALUE) {
                canvasPath.lineTo(path.points[index].x, path.points[index].y)
            } else {
                canvasPath.moveTo(path.points[index].x, path.points[index].y)
            }
        }

        if (canvas != null) {
            for (point in path.points) {
                canvas.drawPath(canvasPath, paint)
            }
        }

        super.onDraw(canvas)
    }
}