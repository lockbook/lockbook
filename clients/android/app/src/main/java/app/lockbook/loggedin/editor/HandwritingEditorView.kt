package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.*
import android.os.Build
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceView
import app.lockbook.R
import app.lockbook.utils.Drawing
import app.lockbook.utils.Event
import app.lockbook.utils.PressurePoint
import app.lockbook.utils.Stroke
import timber.log.Timber

class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    SurfaceView(context, attributeSet), Runnable {
    var drawingModel: Drawing = Drawing()
    private lateinit var canvasBitmap: Bitmap
    private lateinit var tempCanvas: Canvas
    private var thread = Thread(this)
    private var isThreadRunning = false
    private val activePaint = Paint()
    private val lastPoint = PointF()
    private val activePath = Path()
    private val viewPort = Rect()
    private val bitmapPaint = Paint()
    private var isScalling = false
    private var onScreenFocusPoint = PointF()
    private var modelFocusPoint = PointF()
    private var driftX = 0f
    private var driftY = 0f
    private val scaleGestureDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScaleBegin(detector: ScaleGestureDetector?): Boolean {
                    if (detector != null && !isScalling) {
                        isScalling = true
                        onScreenFocusPoint = PointF(detector.focusX, detector.focusY)
                        modelFocusPoint = screenToModel(onScreenFocusPoint)

//                        drawingModel.currentView.transformationrmation.onScreenFocusPoint.x =
//                            zoomFocusPoint.x
//                        drawingModel.currentView.transformation.onScreenFocusPoint.y =
//                            zoomFocusPoint.y

//                        Timber.e("Model: ${zoomFocusPoint}, Screen: (${detector.focusX}, ${detector.focusY}), Scale: ${drawingModel.currentView.transformation.scale}, ViewPort: ${viewPort}")
                    }
                    return true
                }

                override fun onScale(detector: ScaleGestureDetector): Boolean {

                    drawingModel.currentView.transformation.scale *= detector.scaleFactor // 2.14

                    val screenLocationNormalized = PointF(
                        onScreenFocusPoint.x / tempCanvas.clipBounds.width(), // 0.6
                        onScreenFocusPoint.y / tempCanvas.clipBounds.height() // 0.5
                    )

                    val currentViewPortWidth =
                        tempCanvas.clipBounds.width() / drawingModel.currentView.transformation.scale // 1752 / 2.14 = 818.691588785
                    val currentViewPortHeight =
                        tempCanvas.clipBounds.height() / drawingModel.currentView.transformation.scale // 2613 / 2.14 = 1221.028037383

                    driftX = (onScreenFocusPoint.x - detector.focusX) / drawingModel.currentView.transformation.scale
                    driftY = (onScreenFocusPoint.y - detector.focusY) / drawingModel.currentView.transformation.scale

                    val left =
                        ((modelFocusPoint.x + (1 - screenLocationNormalized.x) * currentViewPortWidth) - currentViewPortWidth) + driftX
                    val top =
                        ((modelFocusPoint.y + (1 - screenLocationNormalized.y) * currentViewPortHeight) - currentViewPortHeight) + driftY
                    val right = left + currentViewPortWidth
                    val bottom = top + currentViewPortHeight

                    viewPort.set(left.toInt(), top.toInt(), right.toInt(), bottom.toInt())

                    drawingModel.currentView.transformation.translation.x = -left
                    drawingModel.currentView.transformation.translation.y = -top

                    return true
                }

                override fun onScaleEnd(detector: ScaleGestureDetector?) {
                    Timber.e("scale ended")
                    isScalling = false
                    driftX = 0f
                    driftY = 0f
                    super.onScaleEnd(detector)
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
                drawingModel.currentView.transformation.translation.x += -distanceX
                drawingModel.currentView.transformation.translation.y += -distanceY
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

        bitmapPaint.strokeCap = Paint.Cap.ROUND
        bitmapPaint.strokeJoin = Paint.Join.ROUND
    }

    var currentScale = drawingModel.currentView.transformation.scale

    private fun render(canvas: Canvas) {
        canvas.save()
        canvas.scale(
            drawingModel.currentView.transformation.scale,
            drawingModel.currentView.transformation.scale,
        )

        canvas.translate(
            drawingModel.currentView.transformation.translation.x,
            drawingModel.currentView.transformation.translation.y
        )

        canvas.drawColor(
            Color.TRANSPARENT,
            PorterDuff.Mode.CLEAR
        )
        canvas.drawBitmap(canvasBitmap, 0f, 0f, bitmapPaint)
        canvas.drawCircle(
            modelFocusPoint.x,
            modelFocusPoint.y,
            10f,
            activePaint
        )
        canvas.restore()
    }

    private fun initializeCanvasesAndBitmaps() {
        val canvas = if (Build.VERSION.SDK_INT > Build.VERSION_CODES.N_MR1) {
            holder.lockHardwareCanvas()
        } else {
            holder.lockCanvas()
        }
        canvasBitmap =
            Bitmap.createBitmap(canvas.width, canvas.height, Bitmap.Config.ARGB_8888)
        tempCanvas = Canvas(canvasBitmap)
        val currentPaint = Paint()
        currentPaint.color = Color.WHITE
        currentPaint.strokeWidth = 10f
        currentPaint.style = Paint.Style.STROKE
        tempCanvas.drawRect(Rect(0, 0, tempCanvas.width, tempCanvas.height), currentPaint)
        viewPort.set(canvas.clipBounds)
        holder.unlockCanvasAndPost(canvas)
    }

    private fun restoreFromModel() {
        val currentPaint = Paint()
        currentPaint.isAntiAlias = true
        currentPaint.style = Paint.Style.STROKE
        currentPaint.strokeJoin = Paint.Join.ROUND
        currentPaint.strokeCap = Paint.Cap.ROUND

        for (event in drawingModel.events) {
            if (event.stroke is Stroke) {
                currentPaint.color = event.stroke.color

                for (pointIndex in 0 until event.stroke.points.size) {
                    currentPaint.strokeWidth = event.stroke.points[pointIndex].pressure
                    if (pointIndex != 0) {
                        activePath.moveTo(
                            event.stroke.points[pointIndex - 1].x,
                            event.stroke.points[pointIndex - 1].y
                        )
                        activePath.lineTo(
                            event.stroke.points[pointIndex].x,
                            event.stroke.points[pointIndex].y
                        )
                        tempCanvas.drawPath(activePath, currentPaint)
                        activePath.reset()
                    }
                }

                activePath.reset()
            }
        }
    }

    fun initializeWithDrawing(maybeDrawing: Drawing?) {
        initializeCanvasesAndBitmaps()
        if (maybeDrawing != null) {
            this.drawingModel = maybeDrawing
            restoreFromModel()
        }
        isThreadRunning = true
        thread.start()
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            Timber.e("pointers ${event.pointerCount}")
            if (event.pointerCount > 0) {
                if (event.getToolType(0) == MotionEvent.TOOL_TYPE_STYLUS ||
                    event.getToolType(0) == MotionEvent.TOOL_TYPE_ERASER
                ) {
                    handleStylusEvent(event)
                }
                if (event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER) {
                    handleFingerEvent(event)
                }
            }
        }

        return true
    }

    private fun handleFingerEvent(event: MotionEvent) {
        scaleGestureDetector.onTouchEvent(event)
//        gestureDetector.onTouchEvent(event)
    }

    private fun handleStylusEvent(event: MotionEvent) {
        val modelPoint = screenToModel(PointF(event.x, event.y))
        when (event.action) {
            MotionEvent.ACTION_DOWN -> moveTo(modelPoint, event.pressure)
            MotionEvent.ACTION_MOVE -> lineTo(modelPoint, event.pressure)
        }
    }

    private fun moveTo(point: PointF, pressure: Float) {
        lastPoint.set(point.x, point.y)
        val penPath = Stroke(activePaint.color)
        penPath.points.add(
            PressurePoint(
                point.x,
                point.y,
                pressure * 7 // TODO: This should become a setting, maybe called sensitivity
            )
        )
        drawingModel.events.add(Event(penPath))
    }

    private fun lineTo(point: PointF, pressure: Float) {
        activePaint.strokeWidth = pressure * 7
        activePath.moveTo(
            lastPoint.x,
            lastPoint.y
        )

        activePath.lineTo(
            point.x,
            point.y
        )

        tempCanvas.drawPath(activePath, activePaint)

        activePath.reset()
        lastPoint.set(point.x, point.y)
        for (eventIndex in drawingModel.events.size - 1 downTo 1) {
            val currentEvent = drawingModel.events[eventIndex].stroke
            if (currentEvent is Stroke) {
                currentEvent.points.add(PressurePoint(point.x, point.y, pressure * 7))
                break
            }
        }
    }

    fun screenToModel(screen: PointF): PointF {
        var modelX =
            (viewPort.width() * (screen.x / tempCanvas.clipBounds.width())) + viewPort.left

        if (modelX < 0) modelX = 0f
        if (modelX > tempCanvas.clipBounds.width()) modelX =
            tempCanvas.clipBounds.width().toFloat()

        var modelY =
            (viewPort.height() * (screen.y / tempCanvas.clipBounds.height())) + viewPort.top
        if (modelY < 0) modelY = 0f
        if (modelY > tempCanvas.clipBounds.height()) modelY =
            tempCanvas.clipBounds.height().toFloat()

        return PointF(modelX, modelY)
    }

    fun setColor(color: String) {
        when (color) {
            resources.getString(R.string.handwriting_editor_pallete_white) ->
                activePaint.color =
                    Color.WHITE
            resources.getString(R.string.handwriting_editor_pallete_blue) ->
                activePaint.color =
                    Color.BLUE
            resources.getString(R.string.handwriting_editor_pallete_red) ->
                activePaint.color =
                    Color.RED
            resources.getString(R.string.handwriting_editor_pallete_yellow) ->
                activePaint.color =
                    Color.YELLOW
        }
    }

    fun endThread() {
        isThreadRunning = false
    }

    fun restartThread() {
        isThreadRunning = true
        thread = Thread(this)
        thread.start()
    }

    override fun run() {
        while (isThreadRunning) {
            var canvas: Canvas? = null
            try {
                canvas = if (Build.VERSION.SDK_INT > Build.VERSION_CODES.N_MR1) {
                    holder.lockHardwareCanvas()
                } else {
                    holder.lockCanvas()
                }
                render(canvas)
            } finally { // TODO what happens to this unhandled catch?
                holder.unlockCanvasAndPost(canvas)
            }
        }

        thread.interrupt()
    }
}
