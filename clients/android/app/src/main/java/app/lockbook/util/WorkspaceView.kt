package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.graphics.PointF
import android.graphics.Rect
import android.view.ActionMode
import android.view.Choreographer
import android.view.GestureDetector
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.widget.OverScroller
import android.widget.Toast
import androidx.core.content.ContextCompat.startActivity
import androidx.core.net.toUri
import androidx.input.motionprediction.MotionEventPredictor
import app.lockbook.App
import app.lockbook.model.WorkspaceTabType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.screen.WorkspaceTextInputWrapper
import app.lockbook.workspace.AndroidResponse
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.isNullUUID
import app.lockbook.workspace.toModelTab
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import net.lockbook.Lb

@SuppressLint("ViewConstructor", "SoonBlockedPrivateApi")
class WorkspaceView(
    context: Context,
    val model: WorkspaceViewModel,
) : SurfaceView(context),
    SurfaceHolder.Callback2 {
    private var surface: Surface? = null
    var wrapperView: View? = null
    var contextMenu: ActionMode? = null

    private var redrawTask: Runnable =
        Runnable {
            invalidate()
        }
    private val choreographer: Choreographer by lazy { Choreographer.getInstance() }

    private val ioScope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    private val scroller = OverScroller(context)
    private var gestureStartPositions: Array<PointF> = emptyArray()
    private var pendingDx = 0f
    private var pendingDy = 0f
    private var propagateFlick = false

    private val scrollListener =
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(e: MotionEvent): Boolean {
                scroller.abortAnimation()
                pendingDx = 0f
                pendingDy = 0f
                gestureStartPositions = Array(e.pointerCount) { i -> PointF(e.getX(i), e.getY(i)) }
                propagateFlick = false
                return true
            }

            override fun onScroll(
                e1: MotionEvent?,
                e2: MotionEvent,
                distanceX: Float,
                distanceY: Float,
            ): Boolean {
                if (e2.getToolType(0) == MotionEvent.TOOL_TYPE_STYLUS ||
                    (!isPenOnlyDraw() && e2.pointerCount == 1)
                ) {
                    return false
                }

                propagateFlick = true

                pendingDx -= distanceX
                pendingDy -= distanceY
                return true
            }

            override fun onFling(
                e1: MotionEvent?,
                e2: MotionEvent,
                velocityX: Float,
                velocityY: Float,
            ): Boolean {
                if (!propagateFlick) {
                    return false
                }

                scroller.fling(
                    0,
                    0,
                    velocityX.toInt(),
                    velocityY.toInt(),
                    Int.MIN_VALUE,
                    Int.MAX_VALUE,
                    Int.MIN_VALUE,
                    Int.MAX_VALUE,
                )

                var lastX = 0
                var lastY = 0

                fun tick(frameTimeNanos: Long) {
                    if (scroller.computeScrollOffset() && isAttachedToWindow) {
                        val dx = (scroller.currX - lastX).toFloat()
                        val dy = (scroller.currY - lastY).toFloat()
                        lastX = scroller.currX
                        lastY = scroller.currY
                        pendingDx += dx
                        pendingDy += dy
                        invalidate()
                        choreographer.postFrameCallback(::tick)
                    }
                }
                choreographer.postFrameCallback(::tick)
                return true
            }
        }
    private val scrollDetector: GestureDetector = GestureDetector(context, scrollListener)

    private var pendingZoom = 1f
    private var pendingFocusX = 0f
    private var pendingFocusY = 0f

    private val scaleListener =
        object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                pendingZoom = 1f
                val halfSpanX = detector.currentSpanX / 2f
                val halfSpanY = detector.currentSpanY / 2f
                gestureStartPositions =
                    arrayOf(
                        PointF(detector.focusX - halfSpanX, detector.focusY - halfSpanY),
                        PointF(detector.focusX + halfSpanX, detector.focusY + halfSpanY),
                    )
                return true
            }

            override fun onScale(detector: ScaleGestureDetector): Boolean {
                pendingZoom *= detector.scaleFactor
                pendingFocusX = detector.focusX
                pendingFocusY = detector.focusY
                return true
            }

            override fun onScaleEnd(detector: ScaleGestureDetector) {
                super.onScaleEnd(detector)
            }
        }

    val tapListener =
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDoubleTap(e: MotionEvent): Boolean {
                cancelTouches(e)
                return true
            }
        }

    private val tapDetector: GestureDetector = GestureDetector(context, tapListener)
    private val scaleDetector: ScaleGestureDetector = ScaleGestureDetector(context, scaleListener)

    init {
        holder.addCallback(this)
        holder.setFormat(PixelFormat.TRANSPARENT)
    }

    var motionEventPredictor = MotionEventPredictor.newInstance(this)

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            requestFocus()
            motionEventPredictor.record(event)

            scaleDetector.onTouchEvent(event)
            scrollDetector.onTouchEvent(event)
            tapDetector.onTouchEvent(event)

            forwardedTouchEvent(event, 0f)

            // if they tap outside the toolbar, we want to refocus the text editor to regain text input
            if (model.currentTab.value
                    ?.type
                    ?.isTextEdit() ?: true
            ) {
                wrapperView?.requestFocus()
            }
        }

        return true
    }

    override fun onHoverEvent(event: MotionEvent): Boolean {
        if (event.getToolType(0) == MotionEvent.TOOL_TYPE_STYLUS) {
            when (event.actionMasked) {
                MotionEvent.ACTION_HOVER_MOVE -> {
                    Workspace.mouseMoved(
                        wgpuObj,
                        event.getX(event.actionIndex),
                        event.getY(event.actionIndex),
                    )
                    invalidate()
                }
            }
            return true
        }
        return super.onHoverEvent(event)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        surface = holder.surface

        wgpuObj =
            Workspace.initWSOffloaded(
                surface!!,
                Lb.lb,
                (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES,
            )

        setWillNotDraw(false)

        isFocusable = true
        isFocusableInTouchMode = true

        requestFocus()
    }

    override fun surfaceChanged(
        holder: SurfaceHolder,
        format: Int,
        width: Int,
        height: Int,
    ) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.resizeWS(
            wgpuObj,
            holder.surface,
            context.resources.displayMetrics.scaledDensity,
        )
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        handler?.removeCallbacks(redrawTask)
        surface = null
    }

    override fun onDetachedFromWindow() {
        handler?.removeCallbacks(redrawTask)
        ioScope.cancel()
        super.onDetachedFromWindow()
    }

    fun setBottomInset(inset: Int) {
        if (wgpuObj != Long.MAX_VALUE && surface != null) {
            Workspace.setBottomInset(
                wgpuObj,
                inset,
            )
        }
        invalidate()
    }

    fun drawWorkspace() {
        if (wgpuObj == Long.MAX_VALUE || surface == null || surface?.isValid != true) {
            return
        }

        if (pendingDx != 0f || pendingDy != 0f || pendingZoom != 1f) {
            Workspace.multiTouch(
                wgpuObj,
                pendingDx,
                pendingDy,
                pendingZoom,
                pendingFocusX,
                pendingFocusY,
                gestureStartPositions.map { it.x }.toFloatArray(),
                gestureStartPositions.map { it.y }.toFloatArray(),
            )
        }

        pendingDx = 0f
        pendingDy = 0f
        pendingZoom = 1f
        pendingFocusX = 0f
        pendingFocusY = 0f

        val response: AndroidResponse = Workspace.enterFrameOffloaded(wgpuObj)

        if (response.urlOpened.isNotEmpty()) {
            try {
                val browserIntent = Intent(Intent.ACTION_VIEW, response.urlOpened.toUri())
                startActivity(context, browserIntent, null)
            } catch (err: Exception) {
                Toast.makeText(context, err.message, Toast.LENGTH_SHORT).show()
            }
        }

        if (!response.docCreated.isNullUUID()) {
            model._openFile.postValue(response.docCreated to true)
        }

        if (response.copiedText.isNotEmpty()) {
            (
                App
                    .applicationContext()
                    .getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            ).setPrimaryClip(ClipData.newPlainText("", response.copiedText))
        }

        val currentTab =
            if (response.tabsChanged || !response.selectedFile.isNullUUID()) {
                Workspace.currentTab(wgpuObj).toModelTab()
            } else {
                null
            }

        if (currentTab != null) {
            model._currentTab.value = currentTab
        }

        if (model.currentTab.value?.type == WorkspaceTabType.Markdown) {
            (wrapperView as? WorkspaceTextInputWrapper)?.let { textInputWrapper ->

                if (response.textUpdated && contextMenu != null) {
                    contextMenu?.finish()
                }

                if (response.selectionUpdated) {
                    textInputWrapper.wsInputConnection.notifySelectionUpdated()
                }

                response.virtualKeyboardShown?.let { model._showKeyboard.value = it }

                if (response.hasEditMenu && contextMenu == null) {
                    val actionModeCallback =
                        TextEditorContextMenu(textInputWrapper)

                    contextMenu =
                        this@WorkspaceView.startActionMode(
                            FloatingTextEditorContextMenu(
                                actionModeCallback,
                                response.editMenuX,
                                response.editMenuY,
                            ),
                            ActionMode.TYPE_FLOATING,
                        )
                }
            }
        }

        if (response.redrawIn < 100) {
            invalidate()
        } else {
            handler.postDelayed(redrawTask, response.redrawIn)
        }
    }

    fun drawImmediately() {
        drawWorkspace()
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)

        drawWorkspace()
    }

    fun launchIo(block: suspend () -> Unit) {
        ioScope.launch { block() }
    }

    fun createDocAt(payload: Pair<Boolean, String>) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }
        Workspace.createDocAt(wgpuObj, payload.first, payload.second)

        invalidate()
    }

    fun cancelTouches(event: MotionEvent) {
        for (i in 0 until event.pointerCount) {
            val pointerId = event.getPointerId(i)
            val pressure = event.getPressure(i)
            Workspace.touchesCancelled(
                wgpuObj,
                pointerId,
                event.getX(i),
                event.getY(i),
                pressure,
            )
        }
    }

    fun forwardedTouchEvent(
        event: MotionEvent,
        touchOffsetY: Float,
    ) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }
        val action = event.action and MotionEvent.ACTION_MASK
        val actionIndex = event.actionIndex
        val pressure = getEventPressure(event, actionIndex)

        when (action) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                if (contextMenu != null) {
                    contextMenu!!.finish()
                }

                val pointerId = event.getPointerId(actionIndex)
                Workspace.touchesBegin(
                    wgpuObj,
                    pointerId,
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure,
                )
            }

            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) {
                    val pointerId = event.getPointerId(i)
                    Workspace.touchesMoved(
                        wgpuObj,
                        pointerId,
                        event.getX(i),
                        event.getY(i) + touchOffsetY,
                        getEventPressure(event, i),
                    )
                }
                motionEventPredictor.predict()?.let { predicted ->
                    for (i in 0 until predicted.pointerCount) {
                        Workspace.touchesPredicted(
                            wgpuObj,
                            predicted.getPointerId(i),
                            predicted.getX(i),
                            predicted.getY(i) + touchOffsetY,
                            getEventPressure(predicted, i),
                        )
                    }
                    predicted.recycle()
                }
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val pointerId = event.getPointerId(actionIndex)
                Workspace.touchesEnded(
                    wgpuObj,
                    pointerId,
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure,
                )
            }

            MotionEvent.ACTION_CANCEL -> {
                val pointerId = event.getPointerId(actionIndex)
                Workspace.touchesCancelled(
                    wgpuObj,
                    pointerId,
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure,
                )
            }
        }

        invalidate()
    }

    private fun getEventPressure(
        event: MotionEvent,
        actionIndex: Int,
    ): Float {
        val touchType = event.getToolType(actionIndex)

        val pressure =
            if (touchType == MotionEvent.TOOL_TYPE_STYLUS) {
                event.pressure * 10f // hack: on the z-fold the range is 0-0.1, uplift this to 0-1
            } else {
                Float.NaN
            }
        return pressure
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        drawImmediately()
    }

    fun openDoc(
        id: String,
        newFile: Boolean,
    ) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.openDoc(wgpuObj, id, newFile)
        invalidate()

        return
    }

    fun back(): Boolean {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return false
        }

        val didNavigate = Workspace.back(wgpuObj)
        if (didNavigate) {
            invalidate()
        }

        return didNavigate
    }

    fun forward(): Boolean {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return false
        }

        val didNavigate = Workspace.forward(wgpuObj)
        if (didNavigate) {
            invalidate()
        }

        return didNavigate
    }

    fun canForward(): Boolean {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return false
        }

        return Workspace.canForward(wgpuObj)
    }

    fun isPenOnlyDraw(): Boolean {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return false
        }

        return Workspace.isPenOnlyDraw(wgpuObj)
    }

    fun getTabs(): Array<String> {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return emptyArray()
        }

        return Workspace.getTabs(wgpuObj)
    }

    fun closeDoc(id: String) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.closeDoc(wgpuObj, id)
        invalidate()
    }

    fun closeAllTabs() {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.closeAllTabs(wgpuObj)
    }

    fun fileRenamed(
        id: String,
        name: String,
    ) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.fileRenamed(wgpuObj, id, name)
    }

    companion object {
        var wgpuObj = Long.MAX_VALUE

        const val SPEN_ACTION_DOWN = 211
        const val SPEN_ACTION_MOVE = 213
        const val SPEN_ACTION_UP = 212
    }

    inner class FloatingTextEditorContextMenu(
        private val textEditorContextMenu: TextEditorContextMenu,
        val editMenuX: Float,
        val editMenuY: Float,
    ) : ActionMode.Callback2() {
        override fun onCreateActionMode(
            mode: ActionMode?,
            menu: Menu?,
        ): Boolean = textEditorContextMenu.onCreateActionMode(mode, menu)

        override fun onPrepareActionMode(
            mode: ActionMode?,
            menu: Menu?,
        ): Boolean = textEditorContextMenu.onPrepareActionMode(mode, menu)

        override fun onActionItemClicked(
            mode: ActionMode?,
            item: MenuItem?,
        ): Boolean = textEditorContextMenu.onActionItemClicked(mode, item)

        override fun onDestroyActionMode(mode: ActionMode?) = textEditorContextMenu.onDestroyActionMode(mode)

        override fun onGetContentRect(
            mode: ActionMode?,
            view: View?,
            outRect: Rect?,
        ) {
            outRect!!.set(
                Rect(
                    (editMenuX * context.resources.displayMetrics.scaledDensity).toInt(),
                    (
                        editMenuY *
                            context.resources.displayMetrics.scaledDensity
                    ).toInt(),
                    (editMenuX * context.resources.displayMetrics.scaledDensity).toInt(),
                    (
                        editMenuY *
                            context.resources.displayMetrics.scaledDensity
                    ).toInt(),
                ),
            )
        }
    }

    inner class TextEditorContextMenu(
        private val textInputWrapper: WorkspaceTextInputWrapper,
    ) : ActionMode.Callback {
        override fun onCreateActionMode(
            mode: ActionMode?,
            menu: Menu?,
        ): Boolean {
            if (mode != null) {
                mode.title = null
                mode.subtitle = null
                mode.titleOptionalHint = true
            }

            if (menu != null) {
                populateMenuWithItems(menu)
            }

            return true
        }

        private fun populateMenuWithItems(menu: Menu) {
            if (!textInputWrapper.wsInputConnection.wsEditable
                    .getSelection()
                    .isEmpty()
            ) {
                menu
                    .add(Menu.NONE, android.R.id.cut, 0, "Cut")
                    .setAlphabeticShortcut('x')
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)

                menu
                    .add(Menu.NONE, android.R.id.copy, 1, "Copy")
                    .setAlphabeticShortcut('c')
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            }

            menu
                .add(Menu.NONE, android.R.id.paste, 2, "Paste")
                .setAlphabeticShortcut('v')
                .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)

            menu
                .add(Menu.NONE, android.R.id.selectAll, 3, "Select all")
                .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
        }

        override fun onPrepareActionMode(
            mode: ActionMode?,
            menu: Menu?,
        ): Boolean = true

        override fun onActionItemClicked(
            mode: ActionMode?,
            item: MenuItem?,
        ): Boolean {
            if (item != null) {
                textInputWrapper.wsInputConnection.performContextMenuAction(item.itemId)
            }

            if (contextMenu != null) {
                contextMenu!!.finish()
            }

            return true
        }

        override fun onDestroyActionMode(mode: ActionMode?) {
            contextMenu = null
        }
    }
}
