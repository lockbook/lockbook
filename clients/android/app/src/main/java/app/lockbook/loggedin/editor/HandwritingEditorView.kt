package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.*
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.SurfaceView
import app.lockbook.utils.Event
import app.lockbook.utils.LockbookDrawable
import app.lockbook.utils.PenPath
import app.lockbook.utils.PressurePoint


class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    SurfaceView(context, attributeSet) {
    private val activePaint = Paint()
    private val activePath = Path()
    var lockBookDrawable: LockbookDrawable = LockbookDrawable()

    init {
        activePaint.isAntiAlias = true
        activePaint.style = Paint.Style.STROKE
        activePaint.strokeJoin = Paint.Join.ROUND
        activePaint.color = Color.WHITE

        setZOrderOnTop(true)
        holder.setFormat(PixelFormat.TRANSPARENT)

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
            MotionEvent.ACTION_DOWN -> moveTo(event)
            MotionEvent.ACTION_MOVE -> lineTo(event)
            MotionEvent.ACTION_UP -> activePath.reset()
        }
        return true
    }

    private fun moveTo(event: MotionEvent) {
//        activePath.moveTo(event.x, event.y)
        val penPath = PenPath(activePaint.color)
        penPath.points.add(PressurePoint(event.x, event.y, 10f))
        lockBookDrawable.events.add(Event(penPath))
        drawLockbookDrawable()
    }

    private fun lineTo(event: MotionEvent) {
//        activePaint.strokeWidth = 10f
//        activePath.lineTo(event.x, event.y)
//        val canvas = holder.lockCanvas()
//        canvas.drawPath(activePath, activePaint)
//        holder.unlockCanvasAndPost(canvas)
        for (eventIndex in lockBookDrawable.events.size - 1 downTo 1) {
            val currentEvent = lockBookDrawable.events[eventIndex].penPath
            if (currentEvent is PenPath) {
                currentEvent.points.add(PressurePoint(event.x, event.y, 10f))
                break
            }
        }
        drawLockbookDrawable()


    }

    private fun handleFingerEvent(event: MotionEvent): Boolean {
//        scaleGestureDetector.onTouchEvent(event)
        return true
    }

    fun drawLockbookDrawable() {
        val canvas = holder.lockCanvas()
        canvas.drawColor(Color.TRANSPARENT, PorterDuff.Mode.CLEAR)

        val currentPaint = Paint()
        currentPaint.isAntiAlias = true
        currentPaint.style = Paint.Style.STROKE
        currentPaint.strokeJoin = Paint.Join.ROUND

        for (eventIndex in 0 until lockBookDrawable.events.size) {
            val currentEvent = lockBookDrawable.events[eventIndex]
            if (currentEvent.penPath is PenPath) {
                currentPaint.color = currentEvent.penPath.color

                for (pointIndex in 0 until currentEvent.penPath.points.size) {
                    currentPaint.strokeWidth = 10f
                    if (pointIndex == 0) {
                        activePath.moveTo(
                            currentEvent.penPath.points[pointIndex].x,
                            currentEvent.penPath.points[pointIndex].y
                        )
                    } else {
                        activePath.lineTo(
                            currentEvent.penPath.points[pointIndex].x,
                            currentEvent.penPath.points[pointIndex].y
                        )
                    }
                }

                canvas.drawPath(activePath, currentPaint)
                activePath.reset()
            }
        }

        holder.unlockCanvasAndPost(canvas)
    }


}
