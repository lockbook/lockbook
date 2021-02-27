package app.lockbook.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.*
import android.os.Build
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceView
import androidx.core.content.res.ResourcesCompat
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.*
import app.lockbook.util.Point
import timber.log.Timber
import kotlin.math.absoluteValue
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sqrt

class HandwritingEditorView(context: Context, attributeSet: AttributeSet?) :
    SurfaceView(context, attributeSet), Runnable {
    var drawingModel: Drawing = Drawing()
    private lateinit var canvasBitmap: Bitmap
    private lateinit var tempCanvas: Canvas
    private var erasePoints = Pair(PointF(-1f, -1f), PointF(-1f, -1f))
    private var thread = Thread(this)
    private var isThreadRunning = false
    private var penSizeMultiplier = 7
    var isErasing = false
    var isTouchable = false

    // Current drawing stroke state
    private val strokePaint = Paint()
    private val bitmapPaint = Paint()
    private val backgroundPaint = Paint()
    private val lastPoint = PointF()
    private var rollingAveragePressure = Float.NaN
    private val strokePath = Path()
    private val strokesBounds = mutableListOf<RectF>()

    // Scaling and Viewport state
    private val viewPort = Rect()
    private var onScreenFocusPoint = PointF()
    private var modelFocusPoint = PointF()
    private var driftWhileScalingX = 0f
    private var driftWhileScalingY = 0f

    enum class PenSize {
        SMALL,
        MEDIUM,
        LARGE
    }

    enum class Tool {
        PEN,
        ERASER
    }

    private val scaleGestureDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScaleBegin(detector: ScaleGestureDetector?): Boolean {
                    if (detector != null && isTouchable) {
                        onScreenFocusPoint = PointF(detector.focusX, detector.focusY)
                        modelFocusPoint = screenToModel(onScreenFocusPoint)
                    }
                    return true
                }

                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    if (isTouchable) {
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
                    }

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
        setUpPaint()
    }

    private fun setUpPaint() {
        strokePaint.isAntiAlias = true
        strokePaint.style = Paint.Style.STROKE
        strokePaint.strokeJoin = Paint.Join.ROUND
        strokePaint.color = Color.WHITE
        strokePaint.strokeCap = Paint.Cap.ROUND

        bitmapPaint.strokeCap = Paint.Cap.ROUND
        bitmapPaint.strokeJoin = Paint.Join.ROUND

        backgroundPaint.style = Paint.Style.FILL
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

        backgroundPaint.color = ResourcesCompat.getColor(
            App.instance.resources,
            R.color.drawingUntouchableBackground,
            App.instance.theme
        )

        canvas.drawPaint(backgroundPaint)
        backgroundPaint.color = Color.BLACK
        canvas.drawRect(Rect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT), backgroundPaint)
        canvas.drawBitmap(canvasBitmap, 0f, 0f, bitmapPaint)
        canvas.restore()
    }

    private fun initializeCanvasesAndBitmaps() {
        canvasBitmap = Bitmap.createBitmap(CANVAS_WIDTH, CANVAS_HEIGHT, Bitmap.Config.ARGB_8888)
        tempCanvas = Canvas(canvasBitmap)
    }

    private fun restoreFromModel() {
        val currentPaint = Paint()
        currentPaint.color = Color.WHITE
        currentPaint.strokeWidth = 10f
        currentPaint.style = Paint.Style.STROKE

        currentPaint.isAntiAlias = true
        currentPaint.strokeJoin = Paint.Join.ROUND
        currentPaint.strokeCap = Paint.Cap.ROUND

        for (event in drawingModel.events) {
            if (event.stroke is Stroke) {
                strokesBounds.add(RectF())
                currentPaint.color = event.stroke.color

                var pointIndex = 3
                while (pointIndex < event.stroke.points.size) {
                    val x1 = event.stroke.points[pointIndex - 2]
                    val y1 = event.stroke.points[pointIndex - 1]

                    val x2 = event.stroke.points[pointIndex + 1]
                    val y2 = event.stroke.points[pointIndex + 2]

                    if (pointIndex == 3) {
                        strokesBounds.last().set(x1, y1, x1, y1)
                        updateLastStrokeBounds(x2, y2)
                    } else {
                        updateLastStrokeBounds(x1, y1)
                        updateLastStrokeBounds(x2, y2)
                    }

                    currentPaint.strokeWidth = event.stroke.points[pointIndex]

                    strokePath.moveTo(x1, y1)
                    strokePath.lineTo(x2, y2)

                    tempCanvas.drawPath(strokePath, currentPaint)
                    strokePath.reset()
                    pointIndex += 3
                }

                strokePath.reset()
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

    private fun updateLastStrokeBounds(x: Float, y: Float) {
        val currentStrokeBounds = strokesBounds.last()

        if (x > currentStrokeBounds.right) {
            currentStrokeBounds.right = x
        }

        if (x < currentStrokeBounds.left) {
            currentStrokeBounds.left = x
        }

        if (y < currentStrokeBounds.top) {
            currentStrokeBounds.top = y
        }

        if (y > currentStrokeBounds.bottom) {
            currentStrokeBounds.bottom = y
        }
    }

    private fun doesEraserSegmentIntersectStroke(strokeIndex: Int, x1: Float, y1: Float, x2: Float, y2: Float): Boolean {
        val currentStrokeBounds = strokesBounds[strokeIndex]
        val eraseBounds = RectF()

        if (x1 > x2) {
            eraseBounds.right = x1
            eraseBounds.left = x2
        } else {
            eraseBounds.right = x2
            eraseBounds.left = x1
        }

        if (y1 > y2) {
            eraseBounds.bottom = y1
            eraseBounds.top = y2
        } else {
            eraseBounds.bottom = y2
            eraseBounds.top = y1
        }

        return RectF.intersects(currentStrokeBounds, eraseBounds)
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

        modelX = (modelX * 100).roundToInt() / 100f
        modelY = (modelY * 100).roundToInt() / 100f

        if (modelX < 0) {
            modelX = 0f
        } else if (modelX > CANVAS_WIDTH) {
            modelX = CANVAS_WIDTH.toFloat()
        }

        if (modelY < 0) {
            modelY = 0f
        } else if (modelY > CANVAS_HEIGHT) {
            modelY = CANVAS_HEIGHT.toFloat()
        }

        return PointF(modelX, modelY)
    }

    fun initializeWithDrawing(maybeDrawing: Drawing?) {
        initializeCanvasesAndBitmaps()
        if (maybeDrawing != null) {
            this.drawingModel = maybeDrawing
        }
        restoreFromModel()
        startThread()
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null && isTouchable) {
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

        if (isErasing || event.buttonState == MotionEvent.BUTTON_STYLUS_PRIMARY) {
            if (event.action == SPEN_ACTION_DOWN && (!erasePoints.first.x.isNaN() || !erasePoints.second.x.isNaN())) {
                erasePoints.first.set(PointF(Float.NaN, Float.NaN))
                erasePoints.second.set(PointF(Float.NaN, Float.NaN))
            }

            eraseAtPoint(modelPoint)
        } else {
            when (event.action) {
                MotionEvent.ACTION_DOWN -> moveTo(modelPoint, event.pressure)
                MotionEvent.ACTION_MOVE -> lineTo(modelPoint, event.pressure)
            }
        }
    }

    private fun getAdjustedPressure(pressure: Float): Float = ((pressure * penSizeMultiplier) * 100).roundToInt() / 100f

    private fun moveTo(point: PointF, pressure: Float) {
        lastPoint.set(point)
        strokesBounds.add(RectF(point.x, point.y, point.x, point.y))

        rollingAveragePressure = getAdjustedPressure(pressure)

        val penPath = Stroke(strokePaint.color)

        penPath.points.add(rollingAveragePressure)
        penPath.points.add(point.x)
        penPath.points.add(point.y)
        drawingModel.events.add(Event(penPath))
    }

    private fun approximateRollingAveragePressure(previousRollingAverage: Float, newPressure: Float): Float {
        var newRollingAverage = previousRollingAverage

        newRollingAverage -= newRollingAverage / PRESSURE_SAMPLES_AVERAGED
        newRollingAverage += newPressure / PRESSURE_SAMPLES_AVERAGED

        return newRollingAverage
    }

    private fun lineTo(point: PointF, pressure: Float) {
        val adjustedCurrentPressure = getAdjustedPressure(pressure)
        rollingAveragePressure = approximateRollingAveragePressure(rollingAveragePressure, adjustedCurrentPressure)
        updateLastStrokeBounds(point.x, point.y)

        strokePaint.strokeWidth = rollingAveragePressure

        strokePath.moveTo(
            lastPoint.x,
            lastPoint.y
        )

        strokePath.lineTo(
            point.x,
            point.y
        )

        tempCanvas.drawPath(strokePath, strokePaint)

        strokePath.reset()
        lastPoint.set(point)

        for (eventIndex in drawingModel.events.size - 1 downTo 0) {
            val currentEvent = drawingModel.events[eventIndex].stroke
            if (currentEvent is Stroke) {
                currentEvent.points.add(rollingAveragePressure)
                currentEvent.points.add(point.x)
                currentEvent.points.add(point.y)
                break
            }
        }
    }

    private fun eraseAtPoint(point: PointF) {
        Timber.e("CHECKED: ${strokesBounds.size}")
        val roundedPressure = 20

        when {
            erasePoints.first.x.isNaN() -> {
                erasePoints.first.set(point)
                return
            }
            erasePoints.second.x.isNaN() -> {
                erasePoints.second.set(point)
            }
            else -> {
                erasePoints.first.set(erasePoints.second)
                erasePoints.second.set(point)
            }
        }

        val drawing = Drawing(
            Page(
                Transformation(
                    Point(
                        drawingModel.currentView.transformation.translation.x,
                        drawingModel.currentView.transformation.translation.y
                    ),
                    drawingModel.currentView.transformation.scale,
                )
            ),
            drawingModel.events.map { event ->
                Event(
                    if (event.stroke == null) null else Stroke(
                        event.stroke.color,
                        event.stroke.points.toMutableList()
                    )
                )
            }.toMutableList()
        )

        var refreshScreen = false

        for (eventIndex in drawing.events.size - 1 downTo 0) {
            val stroke = drawing.events[eventIndex].stroke
            if (stroke != null) {
                var deleteStroke = false
                var pointIndex = 3

                val size = (strokesBounds[eventIndex].top - strokesBounds[eventIndex].bottom).absoluteValue * (strokesBounds[eventIndex].left - strokesBounds[eventIndex].right).absoluteValue

                if (!doesEraserSegmentIntersectStroke(eventIndex, erasePoints.first.x, erasePoints.first.y, erasePoints.second.x, erasePoints.second.y) && size > 20) {
                    continue
                }

                pointLoop@ while (pointIndex < stroke.points.size) {
                    for (pixel in 1..roundedPressure) {
                        val roundedPoint1 =
                            PointF(stroke.points[pointIndex - 2].roundToInt().toFloat(), stroke.points[pointIndex - 1].roundToInt().toFloat())
                        val roundedPoint2 =
                            PointF(stroke.points[pointIndex + 1].roundToInt().toFloat(), stroke.points[pointIndex + 2].roundToInt().toFloat())

                        val distBetweenErasePoints = distanceBetweenPoints(erasePoints.first, erasePoints.second)
                        val distToFromRoundedPoint1 = distanceBetweenPoints(erasePoints.first, roundedPoint1) +
                            distanceBetweenPoints(roundedPoint1, erasePoints.second)

                        if (((distToFromRoundedPoint1 - roundedPressure)..(distToFromRoundedPoint1 + roundedPressure)).contains(distBetweenErasePoints)) {
                            deleteStroke = true
                            break@pointLoop
                        }

                        val distToFromRoundedPoint2 = distanceBetweenPoints(erasePoints.first, roundedPoint2) +
                            distanceBetweenPoints(roundedPoint2, erasePoints.second)

                        if (((distToFromRoundedPoint2 - roundedPressure)..(distToFromRoundedPoint2 + roundedPressure)).contains(distBetweenErasePoints)) {
                            deleteStroke = true
                            break@pointLoop
                        }

                        val distBetweenRoundedPoints = distanceBetweenPoints(roundedPoint1, roundedPoint2)
                        val distToFromErasePoint1 = distanceBetweenPoints(roundedPoint1, erasePoints.first) +
                            distanceBetweenPoints(erasePoints.first, roundedPoint2)

                        if (((distToFromErasePoint1 - roundedPressure)..(distToFromErasePoint1 + roundedPressure)).contains(distBetweenRoundedPoints)) {
                            deleteStroke = true
                            break@pointLoop
                        }

                        val distToFromErasePoint2 = distanceBetweenPoints(roundedPoint1, erasePoints.second) +
                            distanceBetweenPoints(erasePoints.second, roundedPoint2)

                        if (((distToFromErasePoint2 - roundedPressure)..(distToFromErasePoint2 + roundedPressure)).contains(distBetweenRoundedPoints)) {
                            deleteStroke = true
                            break@pointLoop
                        }
                    }

                    pointIndex += 3
                }

                if (deleteStroke) {
                    drawing.events.removeAt(eventIndex)
                    refreshScreen = true
                }
            }
        }

        if (refreshScreen) {
            strokesBounds.clear()
            drawingModel = drawing
            tempCanvas.drawColor(
                Color.TRANSPARENT,
                PorterDuff.Mode.CLEAR
            )
            restoreFromModel()
        }
    }

    private fun distanceBetweenPoints(initialPoint: PointF, endPoint: PointF): Float =
        sqrt((initialPoint.x - endPoint.x).pow(2) + (initialPoint.y - endPoint.y).pow(2))

    fun setPenSize(penSize: PenSize) {
        when (penSize) {
            PenSize.SMALL -> penSizeMultiplier = 7
            PenSize.MEDIUM -> penSizeMultiplier = 20
            PenSize.LARGE -> penSizeMultiplier = 40
        }.exhaustive
    }

    fun setColor(colorId: Int) {
        val color = ResourcesCompat.getColor(
            App.instance.resources,
            colorId,
            App.instance.theme
        )
        strokePaint.color = color
    }

    fun endThread() {
        isThreadRunning = false
    }

    fun restartThread() {
        thread = Thread(this)
    }

    fun startThread() {
        isThreadRunning = true
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
            } finally {
                holder.unlockCanvasAndPost(canvas)
            }
        }
    }
}
