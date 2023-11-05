package app.lockbook.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.*
import android.os.Build
import android.util.AttributeSet
import android.view.*
import androidx.core.content.res.ResourcesCompat
import androidx.core.view.GestureDetectorCompat
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.PersistentDrawingInfo
import app.lockbook.screen.MainScreenActivity
import app.lockbook.util.ColorAlias
import app.lockbook.util.Drawing
import app.lockbook.util.Stroke
import timber.log.Timber
import java.lang.ref.WeakReference
import java.util.*
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sqrt

class DrawingView(context: Context, attributeSet: AttributeSet?) :
    SurfaceView(context, attributeSet), Runnable, SurfaceHolder.Callback {

    lateinit var drawing: Drawing
    private lateinit var canvasBitmap: Bitmap
    private lateinit var canvas: Canvas

    lateinit var strokeState: DrawingStrokeState

    private var thread: Thread? = null
    var isThreadAvailable = false
    var isDrawingAvailable = false

    lateinit var colorAliasInARGB: EnumMap<ColorAlias, Int?>

    // Scaling and Viewport state
    private val viewPort = Rect()
    private var onScreenFocusPoint = PointF()
    private var modelFocusPoint = PointF()
    private var driftWhileScalingX = 0f
    private var driftWhileScalingY = 0f

    sealed class Tool {
        object Eraser : Tool()
        data class Pen(val colorAlias: ColorAlias) : Tool()
    }

    private val alertModel by lazy {
        AlertModel(WeakReference(context as MainScreenActivity))
    }

    companion object {
        const val CANVAS_WIDTH = 2125
        const val CANVAS_HEIGHT = 2750

        const val PRESSURE_SAMPLES_AVERAGED = 5
        const val SPEN_ACTION_DOWN = 211
        const val SPEN_ACTION_UP = 212
    }

    private val gestureDetector =
        GestureDetectorCompat(
            context,
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onDown(e: MotionEvent?): Boolean {
                    if (drawing.model.isFingerDrawing && e != null) {
                        handleStylusEvent(e)
                    }

                    return super.onDown(e)
                }

                override fun onScroll(
                    e1: MotionEvent,
                    e2: MotionEvent,
                    distanceX: Float,
                    distanceY: Float
                ): Boolean {
                    if (drawing.model.isFingerDrawing) {
                        handleStylusEvent(e2)
                    } else {
                        drawing.translationX -= distanceX / drawing.scale
                        drawing.translationY -= distanceY / drawing.scale

                        alignViewPortWithBitmap()

                        drawing.justEdited()
                    }

                    return true
                }
            }
        )

    private val scaleGestureDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.OnScaleGestureListener {
                override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                    onScreenFocusPoint = PointF(detector.focusX, detector.focusY)
                    modelFocusPoint = screenToModel(onScreenFocusPoint) ?: return false

                    return true
                }

                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    drawing.scale *= detector.scaleFactor

                    val screenLocationNormalized = PointF(
                        onScreenFocusPoint.x / canvas.clipBounds.width(),
                        onScreenFocusPoint.y / canvas.clipBounds.height()
                    )

                    val currentViewPortWidth =
                        canvas.clipBounds.width() / drawing.scale
                    val currentViewPortHeight =
                        canvas.clipBounds.height() / drawing.scale

                    driftWhileScalingX =
                        (onScreenFocusPoint.x - detector.focusX) / drawing.scale
                    driftWhileScalingY =
                        (onScreenFocusPoint.y - detector.focusY) / drawing.scale

                    val left =
                        ((modelFocusPoint.x + (1 - screenLocationNormalized.x) * currentViewPortWidth) - currentViewPortWidth) + driftWhileScalingX
                    val top =
                        ((modelFocusPoint.y + (1 - screenLocationNormalized.y) * currentViewPortHeight) - currentViewPortHeight) + driftWhileScalingY
                    val right = left + currentViewPortWidth
                    val bottom = top + currentViewPortHeight

                    viewPort.set(left.toInt(), top.toInt(), right.toInt(), bottom.toInt())

                    drawing.translationX = -left
                    drawing.translationY = -top

                    drawing.justEdited()

                    return true
                }

                override fun onScaleEnd(detector: ScaleGestureDetector) {
                    driftWhileScalingX = 0f
                    driftWhileScalingY = 0f
                }
            }
        )

    init {
        holder.setKeepScreenOn(true)
        holder.addCallback(this)
    }

    fun initialize(persistentDrawingInfo: PersistentDrawingInfo) {
        visibility = View.VISIBLE
        this.drawing = persistentDrawingInfo.drawing
        this.canvas = persistentDrawingInfo.canvas
        this.canvasBitmap = persistentDrawingInfo.bitmap
        this.strokeState = persistentDrawingInfo.strokeState

        val emptyBitmap = Bitmap.createBitmap(CANVAS_WIDTH, CANVAS_HEIGHT, Bitmap.Config.ARGB_8888)

        if (persistentDrawingInfo.drawing != Drawing() && persistentDrawingInfo.bitmap.sameAs(emptyBitmap)) {
            restoreBitmapFromDrawing()
        }

        // If the user's theme changed, refresh the entire drawing to account for black white stroke differences
        if (resources.configuration.uiMode != drawing.uiMode) {
            drawing.uiMode = resources.configuration.uiMode
            restoreBitmapFromDrawing()
        }

        alignViewPortWithBitmap()

        isDrawingAvailable = true
        if (isThreadAvailable) {
            startThread()
        }
    }

    private fun render(canvas: Canvas) {
        canvas.save()
        canvas.scale(
            drawing.scale,
            drawing.scale,
        )

        canvas.translate(
            drawing.translationX,
            drawing.translationY
        )

        strokeState.backgroundPaint.color = ResourcesCompat.getColor(
            resources,
            R.color.md_theme_outline,
            context.theme
        )

        canvas.drawPaint(strokeState.backgroundPaint)

        strokeState.backgroundPaint.color = ResourcesCompat.getColor(
            resources,
            R.color.md_theme_inverseOnSurface,
            context.theme
        )

        canvas.drawRect(Rect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT), strokeState.backgroundPaint)
        canvas.drawBitmap(canvasBitmap, 0f, 0f, strokeState.bitmapPaint)
        canvas.restore()
    }

    private fun getColor(colorAlias: ColorAlias, alpha: Float): Int? {
        val alphaAsInt = (alpha * 255).toInt()

        return if (alphaAsInt == 255) {
            colorAliasInARGB[colorAlias]
        } else {
            drawing.getARGBColor(resources.configuration.uiMode, colorAlias, alphaAsInt)
        }
    }

    private fun restoreBitmapFromDrawing() {
        val restoreBitmap = Bitmap.createBitmap(CANVAS_WIDTH, CANVAS_HEIGHT, Bitmap.Config.ARGB_8888)
        val restoreCanvas = Canvas(restoreBitmap)

        for (stroke in drawing.strokes) {
            val strokeColor = getColor(stroke.color, stroke.alpha)

            if (strokeColor == null) {
                alertModel.notifyBasicError()
                return
            }

            strokeState.strokesBounds.add(RectF())
            strokeState.strokePaint.color = strokeColor

            for (pointIndex in 0..(stroke.pointsX.size - 2)) {
                val x1 = stroke.pointsX[pointIndex]
                val y1 = stroke.pointsY[pointIndex]

                val x2 = stroke.pointsX[pointIndex + 1]
                val y2 = stroke.pointsY[pointIndex + 1]

                val pointWidth1 = stroke.pointsGirth[pointIndex]
                val pointWidth2 = stroke.pointsGirth[pointIndex + 1]
                if (pointIndex == 0) {
                    strokeState.strokesBounds.last()
                        .set(x1 - pointWidth1, y1 - pointWidth1, x1 + pointWidth1, y1 + pointWidth1)
                    updateLastStrokeBounds(x2, y2, pointWidth2)
                } else {
                    updateLastStrokeBounds(x1, y1, pointWidth1)
                    updateLastStrokeBounds(x2, y2, pointWidth2)
                }

                strokeState.apply {
                    strokePaint.strokeWidth = pointWidth1
                    strokePath.moveTo(
                        x1,
                        y1
                    )
                    strokePath.lineTo(
                        x2,
                        y2
                    )
                    restoreCanvas.drawPath(strokePath, strokePaint)
                    strokePath.reset()
                }
            }

            strokeState.strokePath.reset()
        }

        val strokeColor = colorAliasInARGB[ColorAlias.White]

        if (strokeColor == null) {
            alertModel.notifyBasicError()
            return
        }

        strokeState.strokePaint.color = strokeColor

        setNewBitmap(restoreBitmap, restoreCanvas)
        alignViewPortWithBitmap()
    }

    private fun setNewBitmap(
        newBitmap: Bitmap,
        newCanvas: Canvas
    ) {
        canvasBitmap = newBitmap
        drawing.model.persistentDrawingInfo.bitmap = newBitmap

        canvas = newCanvas
        drawing.model.persistentDrawingInfo.canvas = newCanvas
    }

    private fun alignViewPortWithBitmap() {
        val currentViewPortWidth =
            canvas.clipBounds.width() / drawing.scale
        val currentViewPortHeight =
            canvas.clipBounds.height() / drawing.scale
        viewPort.left = -drawing.translationX.toInt()
        viewPort.top = -drawing.translationY.toInt()
        viewPort.right = (viewPort.left + currentViewPortWidth).toInt()
        viewPort.bottom = (viewPort.top + currentViewPortHeight).toInt()
    }

    private fun updateLastStrokeBounds(x: Float, y: Float, pointWidth: Float) {
        val currentStrokeBounds = strokeState.strokesBounds.last()
        val left = x - pointWidth
        val top = y - pointWidth
        val right = x + pointWidth
        val bottom = y + pointWidth

        if (right > currentStrokeBounds.right) {
            currentStrokeBounds.right = right
        } else if (left < currentStrokeBounds.left) {
            currentStrokeBounds.left = left
        }

        if (top < currentStrokeBounds.top) {
            currentStrokeBounds.top = top
        } else if (bottom > currentStrokeBounds.bottom) {
            currentStrokeBounds.bottom = bottom
        }
    }

    private fun doesEraserSegmentIntersectStroke(
        x1: Float,
        y1: Float,
        x2: Float,
        y2: Float,
        strokeIndex: Int
    ): Boolean {
        val currentStrokeBounds = strokeState.strokesBounds[strokeIndex]
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

        // expand the erasing bounds to catch the small strokes (like dots) that would not be caught otherwise
        eraseBounds.top -= 20
        eraseBounds.bottom += 20
        eraseBounds.left -= 20
        eraseBounds.right += 20

        return RectF.intersects(currentStrokeBounds, eraseBounds)
    }

    private fun screenToModel(screen: PointF): PointF? {
        var modelX =
            (viewPort.width() * (screen.x / canvas.clipBounds.width())) + viewPort.left
        var modelY =
            (viewPort.height() * (screen.y / canvas.clipBounds.height())) + viewPort.top

        if (modelX.isNaN() || modelY.isNaN()) {
            return null
        }

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

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        Timber.e("touching drawing...")
        if (event != null) {
            val toolType = event.getToolType(0)

            if (toolType == MotionEvent.TOOL_TYPE_STYLUS ||
                toolType == MotionEvent.TOOL_TYPE_ERASER
            ) {
                handleStylusEvent(event)
            }
            if (toolType == MotionEvent.TOOL_TYPE_FINGER) {
                handleFingerEvent(event)
            }
        }

        return true
    }

    private fun handleFingerEvent(event: MotionEvent) {
        gestureDetector.onTouchEvent(event)
        if (!drawing.model.isFingerDrawing) {
            scaleGestureDetector.onTouchEvent(event)
        }
    }

    private fun handleStylusEvent(event: MotionEvent) {
        val modelPoint = screenToModel(PointF(event.x, event.y)) ?: return
        val action = event.action

        strokeState.apply {
            if (action == SPEN_ACTION_DOWN || (action == MotionEvent.ACTION_DOWN && (event.buttonState == MotionEvent.BUTTON_STYLUS_PRIMARY || event.getToolType(0) == MotionEvent.TOOL_TYPE_ERASER))) { // stay erasing if the button isn't held but it is the same stroke && vice versa
                penState = PenState.ErasingWithPen
            } else if (penState == PenState.ErasingWithPen && (action == MotionEvent.ACTION_UP || action == SPEN_ACTION_UP)) {
                penState = PenState.Drawing
            }

            if (penState == PenState.ErasingWithPen || penState == PenState.ErasingWithTouchButton) {
                if ((action == SPEN_ACTION_DOWN || action == MotionEvent.ACTION_DOWN) && (!erasePoints.first.x.isNaN() || !erasePoints.second.x.isNaN())) {
                    erasePoints.first.set(PointF(Float.NaN, Float.NaN))
                    erasePoints.second.set(PointF(Float.NaN, Float.NaN))
                }

                eraseAtPoint(modelPoint)
            } else {
                when (action) {
                    MotionEvent.ACTION_DOWN -> moveTo(modelPoint, event.pressure)
                    MotionEvent.ACTION_MOVE -> lineTo(modelPoint, event.pressure)
                }

                drawing.justEdited()
            }
        }
    }

    private fun getAdjustedPressure(pressure: Float): Float =
        ((pressure * strokeState.penSizeMultiplier) * 100).roundToInt() / 100f

    private fun moveTo(point: PointF, pressure: Float) {
        strokeState.apply {
            lastPoint.set(point)
            rollingAveragePressure = getAdjustedPressure(pressure)

            val boundsAdjustedForPressure = RectF(
                point.x - rollingAveragePressure,
                point.y - rollingAveragePressure,
                point.x + rollingAveragePressure,
                point.y + rollingAveragePressure
            )
            strokesBounds.add(boundsAdjustedForPressure)

            val strokeColor = getColor(strokeColor, alpha)

            if (strokeColor == null) {
                alertModel.notifyBasicError()
                return
            }

            strokePaint.color = strokeColor

            val stroke = Stroke(
                mutableListOf(point.x),
                mutableListOf(point.y),
                mutableListOf(rollingAveragePressure),
                this.strokeColor,
                strokeAlpha.toFloat() / 255
            )

            drawing.strokes.add(stroke)
        }
    }

    private fun approximateRollingAveragePressure(
        previousRollingAverage: Float,
        newPressure: Float
    ): Float {
        var newRollingAverage = previousRollingAverage

        newRollingAverage -= newRollingAverage / PRESSURE_SAMPLES_AVERAGED
        newRollingAverage += newPressure / PRESSURE_SAMPLES_AVERAGED

        return newRollingAverage
    }

    private fun lineTo(point: PointF, pressure: Float) {
        strokeState.apply {
            if (lastPoint.equals(
                    Float.NaN,
                    Float.NaN
                )
            ) { // if you start drawing after just erasing, and the pen was never lifted, this will compensate for it
                return moveTo(point, pressure)
            }

            val adjustedCurrentPressure = getAdjustedPressure(pressure)

            rollingAveragePressure =
                approximateRollingAveragePressure(rollingAveragePressure, adjustedCurrentPressure)
            updateLastStrokeBounds(point.x, point.y, rollingAveragePressure)

            strokePaint.strokeWidth = rollingAveragePressure

            strokePath.moveTo(
                lastPoint.x,
                lastPoint.y
            )

            strokePath.lineTo(
                point.x,
                point.y
            )

            canvas.drawPath(strokePath, strokePaint)

            strokePath.reset()
            lastPoint.set(point)

            drawing.strokes.last { stroke ->
                stroke.pointsX.add(point.x)
                stroke.pointsY.add(point.y)
                stroke.pointsGirth.add(rollingAveragePressure)
            }
        }
    }

    private fun eraseAtPoint(point: PointF) {
        strokeState.apply {
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
        }

        if (!strokeState.lastPoint.equals(Float.NaN, Float.NaN)) {
            strokeState.lastPoint.set(Float.NaN, Float.NaN)
        }

        val drawingClone = drawing.clone()

        var refreshScreen = false

        for (strokeIndex in drawingClone.strokes.size - 1 downTo 0) {
            val stroke = drawingClone.strokes[strokeIndex]
            var deleteStroke = false

            if (!doesEraserSegmentIntersectStroke(
                    strokeState.erasePoints.first.x,
                    strokeState.erasePoints.first.y,
                    strokeState.erasePoints.second.x,
                    strokeState.erasePoints.second.y,
                    strokeIndex
                )
            ) {
                continue
            }

            pointLoop@ for (pointIndex in 0..(stroke.pointsX.size - 2)) {
                if (pointIndex < stroke.pointsX.size - 1) {
                    var roundedPressure = stroke.pointsGirth[pointIndex].toInt()

                    if (roundedPressure < 5) {
                        roundedPressure = 5
                    }

                    for (pixel in 1..roundedPressure) {
                        val roundedPoint1 =
                            PointF(
                                stroke.pointsX[pointIndex].roundToInt().toFloat(),
                                stroke.pointsY[pointIndex].roundToInt().toFloat()
                            )
                        val roundedPoint2 =
                            PointF(
                                stroke.pointsX[pointIndex + 1].roundToInt().toFloat(),
                                stroke.pointsY[pointIndex + 1].roundToInt().toFloat()
                            )

                        val distBetweenErasePoints =
                            distanceBetweenPoints(strokeState.erasePoints.first, strokeState.erasePoints.second)
                        val distToFromRoundedPoint1 =
                            distanceBetweenPoints(strokeState.erasePoints.first, roundedPoint1) +
                                distanceBetweenPoints(roundedPoint1, strokeState.erasePoints.second)

                        if (((distToFromRoundedPoint1 - roundedPressure)..(distToFromRoundedPoint1 + roundedPressure)).contains(
                                distBetweenErasePoints
                            )
                        ) {
                            deleteStroke = true
                            break@pointLoop
                        }

                        val distToFromRoundedPoint2 =
                            distanceBetweenPoints(strokeState.erasePoints.first, roundedPoint2) +
                                distanceBetweenPoints(roundedPoint2, strokeState.erasePoints.second)

                        if (((distToFromRoundedPoint2 - roundedPressure)..(distToFromRoundedPoint2 + roundedPressure)).contains(
                                distBetweenErasePoints
                            )
                        ) {
                            deleteStroke = true
                            break@pointLoop
                        }

                        val distBetweenRoundedPoints =
                            distanceBetweenPoints(roundedPoint1, roundedPoint2)
                        val distToFromErasePoint1 =
                            distanceBetweenPoints(roundedPoint1, strokeState.erasePoints.first) +
                                distanceBetweenPoints(strokeState.erasePoints.first, roundedPoint2)

                        if (((distToFromErasePoint1 - roundedPressure)..(distToFromErasePoint1 + roundedPressure)).contains(
                                distBetweenRoundedPoints
                            )
                        ) {
                            deleteStroke = true
                            break@pointLoop
                        }

                        val distToFromErasePoint2 =
                            distanceBetweenPoints(roundedPoint1, strokeState.erasePoints.second) +
                                distanceBetweenPoints(strokeState.erasePoints.second, roundedPoint2)

                        if (((distToFromErasePoint2 - roundedPressure)..(distToFromErasePoint2 + roundedPressure)).contains(
                                distBetweenRoundedPoints
                            )
                        ) {
                            deleteStroke = true
                            break@pointLoop
                        }
                    }
                }
            }

            if (deleteStroke) {
                drawingClone.strokes.removeAt(strokeIndex)
                refreshScreen = true
            }
        }

        if (refreshScreen) {
            drawing.set(drawingClone)
            drawing.justEdited()

            strokeState.strokesBounds.clear()

            restoreBitmapFromDrawing()
        }
    }

    private fun distanceBetweenPoints(initialPoint: PointF, endPoint: PointF): Float =
        sqrt((initialPoint.x - endPoint.x).pow(2) + (initialPoint.y - endPoint.y).pow(2))

    fun startThread() {
        if (holder.surface.isValid && thread == null) {
            thread = Thread(this)
            isThreadAvailable = true
            thread!!.start()
        }
    }

    fun stopThread() {
        if (thread == null) {
            return
        }

        isThreadAvailable = false
        while (thread?.isAlive == true) {
            try {
                thread?.join() ?: return
            } catch (e: Exception) {
            }
        }

        thread = null
    }

    override fun run() {
        while (isThreadAvailable && isDrawingAvailable) {
            if (holder == null) {
                return
            }
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

    override fun surfaceCreated(holder: SurfaceHolder) {
        if (thread != null) {
            stopThread()
        }

        isThreadAvailable = true
        if (isDrawingAvailable) {
            startThread()
        }
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        stopThread()
        holder.surface.release()
    }
}

data class DrawingStrokeState(
    var erasePoints: Pair<PointF, PointF> =
        Pair(PointF(Float.NaN, Float.NaN), PointF(Float.NaN, Float.NaN)),
    var penSizeMultiplier: Int = 7,
    var strokeAlpha: Int = 255,
    var strokeColor: ColorAlias = ColorAlias.White,
    val strokePaint: Paint = Paint(),
    val bitmapPaint: Paint = Paint(),
    val backgroundPaint: Paint = Paint(),
    val lastPoint: PointF = PointF(),
    var rollingAveragePressure: Float = Float.NaN,
    val strokePath: Path = Path(),
    val strokesBounds: MutableList<RectF> = mutableListOf(),
    var penState: PenState = PenState.Drawing
)

enum class PenState {
    Drawing,
    ErasingWithPen,
    ErasingWithTouchButton
}
