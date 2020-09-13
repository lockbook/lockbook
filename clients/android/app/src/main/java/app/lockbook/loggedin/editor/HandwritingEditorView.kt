package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import app.lockbook.utils.Path
import app.lockbook.utils.PointFloat
import com.beust.klaxon.Klaxon
import timber.log.Timber


class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    View(context, attributeSet) {
    private val paint = Paint()
    var activePath: Path = Path()
    var drawn = 0
    var drawnPaths: MutableList<Path> = mutableListOf()
    var reOpened = false
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
                activePath = Path()
                drawn = 0
                activePath.points.add(PointFloat(-100.321f, -100.321f))
                activePath.points.add(PointFloat(event.x, event.y))
            }
            MotionEvent.ACTION_MOVE -> {
                activePath.points.add(PointFloat(event.x, event.y))
                invalidate()
            }
            MotionEvent.ACTION_UP -> {
                drawnPaths.add(activePath)
            }
        }
        return true
    }

    private fun handleFingerEvent(event: MotionEvent): Boolean {
        scaleGestureDetector.onTouchEvent(event)
        return true
    }

    override fun onDraw(canvas: Canvas?) {
        Timber.e(drawn.toString())
        for (index in drawn until activePath.points.size) {
            drawn++
            if (index != 0 && activePath.points[index - 1].x != -100.321f) {
                canvasPath.lineTo(activePath.points[index].x, activePath.points[index].y)
            } else {
                canvasPath.moveTo(activePath.points[index].x, activePath.points[index].y)
            }
        }

        if(reOpened && canvas != null) {
            for(path in drawnPaths) {
                for (index in drawn until path.points.size) {
                    if (index != 0 && path.points[index - 1].x != -100.321f) {
                        canvasPath.lineTo(path.points[index].x, path.points[index].y)
                    } else {
                        canvasPath.moveTo(path.points[index].x, path.points[index].y)
                    }
                }
            }
            canvas.drawPath(canvasPath, paint)
            reOpened = false
        }

        if (canvas != null) {
            for (point in activePath.points) {
                canvas.drawPath(canvasPath, paint)
            }
        }

        super.onDraw(canvas)
    }
}