package app.lockbook.loggedin.editor

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.*
import android.os.Build
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceView
import app.lockbook.R
import app.lockbook.utils.*
import timber.log.Timber

class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    SurfaceView(context, attributeSet), Runnable {
    var drawingModel: Drawing = Drawing()
    private lateinit var canvasBitmap: Bitmap
    private lateinit var tempCanvas: Canvas
    private var thread = Thread(this)
    private var isThreadRunning = false

    // Current drawing stroke state
    private val activePaint = Paint()
    private val lastPoint = PointF()
    private val activePath = Path()
    private val bitmapPaint = Paint()

    // Scaling and Viewport state
    private val viewPort = Rect()
    private var onScreenFocusPoint = PointF()
    private var modelFocusPoint = PointF()
    private var driftWhileScalingX = 0f
    private var driftWhileScalingY = 0f

    private val scaleGestureDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScaleBegin(detector: ScaleGestureDetector?): Boolean {
                    if (detector != null) {
                        onScreenFocusPoint = PointF(detector.focusX, detector.focusY)
                        modelFocusPoint = screenToModel(onScreenFocusPoint)
                    }
                    return true
                }

                override fun onScale(detector: ScaleGestureDetector): Boolean {

                    drawingModel.currentView.transformation.scale *= detector.scaleFactor

                    val screenLocationNormalized = PointF(
                        onScreenFocusPoint.x / tempCanvas.clipBounds.width(),
                        onScreenFocusPoint.y / tempCanvas.clipBounds.height()
                    )

                    val currentViewPortWidth =
                        tempCanvas.clipBounds.width() / drawingModel.currentView.transformation.scale
                    val currentViewPortHeight =
                        tempCanvas.clipBounds.height() / drawingModel.currentView.transformation.scale

                    driftWhileScalingX =
                        (onScreenFocusPoint.x - detector.focusX) / drawingModel.currentView.transformation.scale
                    driftWhileScalingY =
                        (onScreenFocusPoint.y - detector.focusY) / drawingModel.currentView.transformation.scale

                    val left =
                        ((modelFocusPoint.x + (1 - screenLocationNormalized.x) * currentViewPortWidth) - currentViewPortWidth) + driftWhileScalingX
                    val top =
                        ((modelFocusPoint.y + (1 - screenLocationNormalized.y) * currentViewPortHeight) - currentViewPortHeight) + driftWhileScalingY
                    val right = left + currentViewPortWidth
                    val bottom = top + currentViewPortHeight

                    viewPort.set(left.toInt(), top.toInt(), right.toInt(), bottom.toInt())

                    drawingModel.currentView.transformation.translation.x = -left
                    drawingModel.currentView.transformation.translation.y = -top

                    return true
                }

                override fun onScaleEnd(detector: ScaleGestureDetector?) {
                    driftWhileScalingX = 0f
                    driftWhileScalingY = 0f
                    super.onScaleEnd(detector)
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
        canvas.restore()
    }

    private fun initializeCanvasesAndBitmaps() {
        val canvas = if (Build.VERSION.SDK_INT > Build.VERSION_CODES.N_MR1) {
            holder.lockHardwareCanvas()
        } else {
            holder.lockCanvas()
        }
        canvasBitmap =
            Bitmap.createBitmap(CANVAS_SIZE, CANVAS_SIZE, Bitmap.Config.ARGB_8888)
        tempCanvas = Canvas(canvasBitmap)
        val currentPaint = Paint()
        currentPaint.color = Color.WHITE
        currentPaint.strokeWidth = 10f
        currentPaint.style = Paint.Style.STROKE
        tempCanvas.drawRect(Rect(0, 0, tempCanvas.width, tempCanvas.height), currentPaint)
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

        val currentViewPortWidth =
            tempCanvas.clipBounds.width() / drawingModel.currentView.transformation.scale
        val currentViewPortHeight =
            tempCanvas.clipBounds.height() / drawingModel.currentView.transformation.scale
        viewPort.left = -drawingModel.currentView.transformation.translation.x.toInt()
        viewPort.top = -drawingModel.currentView.transformation.translation.y.toInt()
        viewPort.right = (viewPort.left + currentViewPortWidth).toInt()
        viewPort.bottom = (viewPort.top + currentViewPortHeight).toInt()
    }

    private fun screenToModel(screen: PointF): PointF {
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

    fun initializeWithDrawing(maybeDrawing: Drawing?) {
        initializeCanvasesAndBitmaps()
        if (maybeDrawing != null) {
            this.drawingModel = maybeDrawing
        }
        restoreFromModel()
        isThreadRunning = true
        thread.start()
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
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
        for (eventIndex in drawingModel.events.size - 1 downTo 0) {
            val currentEvent = drawingModel.events[eventIndex].stroke
            if (currentEvent is Stroke) {
                currentEvent.points.add(PressurePoint(point.x, point.y, pressure * 7))
                break
            }
        }
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
        thread = Thread(this)
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
