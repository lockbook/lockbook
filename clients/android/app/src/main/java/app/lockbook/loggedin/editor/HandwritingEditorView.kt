package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.*
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.SurfaceView
import app.lockbook.utils.Event
import app.lockbook.utils.Drawing
import app.lockbook.utils.Stroke
import app.lockbook.utils.PressurePoint

class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    SurfaceView(context, attributeSet) {
    private val activePaint = Paint()
    private val lastPoint = PointF()
    private val activePath = Path()
    private lateinit var canvasBitmap: Bitmap
    private lateinit var tempCanvas: Canvas
    var lockBookDrawable: Drawing = Drawing()

    init {
        activePaint.isAntiAlias = true
        activePaint.style = Paint.Style.STROKE
        activePaint.strokeJoin = Paint.Join.ROUND
        activePaint.color = Color.WHITE
        activePaint.strokeCap = Paint.Cap.ROUND
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
        }
        return true
    }

    private fun moveTo(event: MotionEvent) {
        activePath.reset()
        lastPoint.set(event.x, event.y)
        val penPath = Stroke(activePaint.color)
        penPath.points.add(PressurePoint(event.x, event.y, event.pressure * 7)) //TODO: This should become a setting, maybe called sensitivity
        lockBookDrawable.events.add(Event(penPath))
    }

    private fun lineTo(event: MotionEvent) {
        activePaint.strokeWidth = event.pressure * 7
        activePath.moveTo(lastPoint.x, lastPoint.y)
        activePath.lineTo(event.x, event.y)
        tempCanvas.drawPath(activePath, activePaint)

        val canvas = holder.lockCanvas()
        canvas.drawColor(
            Color.TRANSPARENT,
            PorterDuff.Mode.CLEAR
        )
        canvas.drawBitmap(canvasBitmap, Matrix(), null)
        holder.unlockCanvasAndPost(canvas)

        activePath.reset()
        lastPoint.set(event.x, event.y)
        for (eventIndex in lockBookDrawable.events.size - 1 downTo 1) {
            val currentEvent = lockBookDrawable.events[eventIndex].stroke
            if (currentEvent is Stroke) {
                currentEvent.points.add(PressurePoint(event.x, event.y, event.pressure * 7))
                break
            }
        }
    }

    private fun handleFingerEvent(event: MotionEvent): Boolean {
//        scaleGestureDetector.onTouchEvent(event)
        return true
    }

    fun setUpBitmapCanvas() {
        val canvas = holder.lockCanvas()
        canvasBitmap = Bitmap.createBitmap(canvas.width, canvas.height, Bitmap.Config.ARGB_8888)
        tempCanvas = Canvas(canvasBitmap)
        holder.unlockCanvasAndPost(canvas)
    }

    fun drawLockbookDrawable() {
        val currentPaint = Paint()
        currentPaint.isAntiAlias = true
        currentPaint.style = Paint.Style.STROKE
        currentPaint.strokeJoin = Paint.Join.ROUND
        currentPaint.strokeCap = Paint.Cap.ROUND

        for (eventIndex in 0 until lockBookDrawable.events.size) {
            val currentEvent = lockBookDrawable.events[eventIndex]
            if (currentEvent.stroke is Stroke) {
                currentPaint.color = currentEvent.stroke.color

                for (pointIndex in 0 until currentEvent.stroke.points.size) {
                    currentPaint.strokeWidth = currentEvent.stroke.points[pointIndex].pressure
                    if (pointIndex != 0) {
                        activePath.moveTo(
                            currentEvent.stroke.points[pointIndex - 1].x,
                            currentEvent.stroke.points[pointIndex - 1].y
                        )
                        activePath.lineTo(
                            currentEvent.stroke.points[pointIndex].x,
                            currentEvent.stroke.points[pointIndex].y
                        )
                        tempCanvas.drawPath(activePath, currentPaint)
                        activePath.reset()
                    }
                }

                activePath.reset()
            }
        }

        val canvas = holder.lockCanvas()
        canvas.drawBitmap(canvasBitmap, Matrix(), null)
        holder.unlockCanvasAndPost(canvas)
    }
}
