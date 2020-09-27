package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.*
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceView
import app.lockbook.utils.Drawing
import app.lockbook.utils.Event
import app.lockbook.utils.PressurePoint
import app.lockbook.utils.Stroke

class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    SurfaceView(context, attributeSet), Runnable {
    private val activePaint = Paint()
    private val lastPoint = PointF()
    private val activePath = Path()
    private val viewPort = Rect()
    var isThreadRunning = false
    private var previousTime = 0L
    private lateinit var canvasBitmap: Bitmap
    private lateinit var tempCanvas: Canvas
    var lockBookDrawable: Drawing = Drawing()
    private val scaleGestureDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    lockBookDrawable.page.transformation.scale *= detector.scaleFactor
                    lockBookDrawable.page.transformation.scale = 1f.coerceAtLeast(
                        lockBookDrawable.page.transformation.scale.coerceAtMost(5.0f)
                    )

                    lockBookDrawable.page.transformation.translation.x = detector.focusX
                    lockBookDrawable.page.transformation.translation.y = detector.focusY

                    return true
                }
            }
        )

    private val gestureDetector = GestureDetector(
        context,
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onScroll(
                e1: MotionEvent?,
                e2: MotionEvent?,
                distanceX: Float,
                distanceY: Float
            ): Boolean {
//            drawingMatrix.setTranslate(distanceX, distanceY)
//
//            drawBitmap()
                return true
            }
        }
    )

    init {
        activePaint.isAntiAlias = true
        activePaint.style = Paint.Style.STROKE
        activePaint.strokeJoin = Paint.Join.ROUND
        activePaint.color = Color.WHITE
        activePaint.strokeCap = Paint.Cap.ROUND
    }

    private fun drawBitmap(canvas: Canvas) {
        canvas.save()
        canvas.scale(
            lockBookDrawable.page.transformation.scale,
            lockBookDrawable.page.transformation.scale,
            lockBookDrawable.page.transformation.translation.x,
            lockBookDrawable.page.transformation.translation.y
        )
        viewPort.set(canvas.clipBounds)
        canvas.drawColor(
            Color.TRANSPARENT,
            PorterDuff.Mode.CLEAR
        )
        canvas.drawBitmap(canvasBitmap, Matrix(), null)
        canvas.restore()
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            for (point in 0 until event.pointerCount) {
                if (event.getToolType(point) == MotionEvent.TOOL_TYPE_STYLUS ||
                    event.getToolType(point) == MotionEvent.TOOL_TYPE_ERASER
                ) {
                    handleStylusEvent(event)
                }
                if (event.getToolType(point) == MotionEvent.TOOL_TYPE_FINGER) {
                    handleFingerEvent(event)
                }
            }
        }

        return true
    }

    private fun handleFingerEvent(event: MotionEvent) {
        scaleGestureDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)
    }

    private fun handleStylusEvent(event: MotionEvent) {
        when (event.action) {
            MotionEvent.ACTION_DOWN -> moveTo(event.x, event.y, event.pressure)
            MotionEvent.ACTION_MOVE -> lineTo(event.x, event.y, event.pressure)
        }
    }

    private fun moveTo(x: Float, y: Float, pressure: Float) {
        lastPoint.set(x, y)
        val penPath = Stroke(activePaint.color)
        penPath.points.add(
            PressurePoint(
                x,
                y,
                pressure * 7
            )
        ) // TODO: This should become a setting, maybe called sensitivity
        lockBookDrawable.events.add(Event(penPath))
    }

    private fun lineTo(x: Float, y: Float, pressure: Float) {
        activePaint.strokeWidth = pressure * 7
        activePath.moveTo(
            (viewPort.width() * (lastPoint.x / tempCanvas.clipBounds.width())) + viewPort.left,
            (viewPort.height() * (lastPoint.y / tempCanvas.clipBounds.height())) + viewPort.top
        )

        activePath.lineTo(
            (viewPort.width() * (x / tempCanvas.clipBounds.width())) + viewPort.left,
            (viewPort.height() * (y / tempCanvas.clipBounds.height())) + viewPort.top
        )

        tempCanvas.drawPath(activePath, activePaint)

        activePath.reset()
        lastPoint.set(x, y)
        for (eventIndex in lockBookDrawable.events.size - 1 downTo 1) {
            val currentEvent = lockBookDrawable.events[eventIndex].stroke
            if (currentEvent is Stroke) {
                currentEvent.points.add(PressurePoint(x, y, pressure * 7))
                break
            }
        }
    }

    fun setUpBitmapDrawable() {
        val canvas = holder.lockCanvas()
        canvasBitmap = Bitmap.createBitmap(canvas.width, canvas.height, Bitmap.Config.ARGB_8888)
        tempCanvas = Canvas(canvasBitmap)
        viewPort.set(canvas.clipBounds)
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
    }

    override fun run() {
        while (isThreadRunning) {
            val currentTimeMillis = System.currentTimeMillis()
            val elapsedTimeMs: Long = currentTimeMillis - previousTime
            val sleepTimeMs = (1000f / 120 - elapsedTimeMs)

            var canvas = holder.lockCanvas()
            try {
                if (canvas == null) {
                    Thread.sleep(1)
                    continue
                } else if (sleepTimeMs > 0) {
                    Thread.sleep(sleepTimeMs.toLong())
                }
                synchronized(holder) { drawBitmap(canvas) }
            } catch (e: Exception) {
                e.printStackTrace()
            } finally {
                if (canvas != null) {
                    holder.unlockCanvasAndPost(canvas)
                    previousTime = System.currentTimeMillis()
                }
            }
        }
    }
}
