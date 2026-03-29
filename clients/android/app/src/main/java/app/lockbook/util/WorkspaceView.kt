package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
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
import androidx.core.content.ContextCompat.startActivity
import androidx.core.net.toUri
import androidx.input.motionprediction.MotionEventPredictor
import app.lockbook.App
import app.lockbook.model.WorkspaceTabType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.screen.WorkspaceTextInputWrapper
import app.lockbook.workspace.AndroidResponse
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.WsStatus
import app.lockbook.workspace.isNullUUID
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.selects.onTimeout
import kotlinx.coroutines.selects.select
import kotlinx.coroutines.withContext
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import net.lockbook.Lb
import java.util.concurrent.atomic.AtomicReference
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

@SuppressLint("ViewConstructor", "SoonBlockedPrivateApi")
class WorkspaceView(context: Context, val model: WorkspaceViewModel) : SurfaceView(context), SurfaceHolder.Callback2 {
    private var eraserToggledOnByPen = false

    private var surface: Surface? = null
    var wrapperView: View? = null
    var contextMenu: ActionMode? = null

    private val renderScope = CoroutineScope(Dispatchers.Default + SupervisorJob())
    private var renderJob: Job? = null

    private val nativeLock = ReentrantLock()

    private val redrawChannel = Channel<Unit>(Channel.CONFLATED)
    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    private val scroller = OverScroller(context)

    private var gestureStartPositions: Array<PointF> = emptyArray()
    private val pendingDx = AtomicReference(0f)
    private val pendingDy = AtomicReference(0f)
    private var propagateFlick = false

    private val scrollListener = object : GestureDetector.SimpleOnGestureListener() {
        override fun onDown(e: MotionEvent): Boolean {
            scroller.abortAnimation()
            pendingDx.set(0f)
            pendingDy.set(0f)
            gestureStartPositions = Array(e.pointerCount) { i -> PointF(e.getX(i), e.getY(i)) }
            propagateFlick = false
            return true
        }

        override fun onScroll(
            e1: MotionEvent?,
            e2: MotionEvent,
            distanceX: Float,
            distanceY: Float
        ): Boolean {
            if (e2.getToolType(0) == MotionEvent.TOOL_TYPE_STYLUS ||
                !isPenOnlyDraw() && e2.pointerCount == 1
            ) {
                return false
            }

            propagateFlick = true

            pendingDx.getAndUpdate { it - distanceX }
            pendingDy.getAndUpdate { it - distanceY }
            drawImmediately()
            return true
        }

        override fun onFling(
            e1: MotionEvent?,
            e2: MotionEvent,
            velocityX: Float,
            velocityY: Float
        ): Boolean {
            if (!propagateFlick) {
                return false
            }

            scroller.fling(
                0, 0,
                velocityX.toInt(),
                velocityY.toInt(),
                Int.MIN_VALUE, Int.MAX_VALUE,
                Int.MIN_VALUE, Int.MAX_VALUE
            )

            var lastX = 0
            var lastY = 0

            val choreographer = Choreographer.getInstance()
            fun tick(frameTimeNanos: Long) {
                if (scroller.computeScrollOffset() && isAttachedToWindow) {
                    val dx = (scroller.currX - lastX).toFloat()
                    val dy = (scroller.currY - lastY).toFloat()
                    lastX = scroller.currX
                    lastY = scroller.currY
                    pendingDx.getAndUpdate { it + dx }
                    pendingDy.getAndUpdate { it + dy }
                    drawImmediately()
                    choreographer.postFrameCallback(::tick)
                }
            }
            choreographer.postFrameCallback(::tick)
            return true
        }
    }
    private val scrollDetector: GestureDetector = GestureDetector(context, scrollListener)

    private val pendingZoom = AtomicReference(1f)
    private val pendingFocusX = AtomicReference(0f)
    private val pendingFocusY = AtomicReference(0f)

    private val scaleListener = object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
            pendingZoom.set(1f)
            val halfSpanX = detector.currentSpanX / 2f
            val halfSpanY = detector.currentSpanY / 2f
            gestureStartPositions = arrayOf(
                PointF(detector.focusX - halfSpanX, detector.focusY - halfSpanY),
                PointF(detector.focusX + halfSpanX, detector.focusY + halfSpanY)
            )
            return true
        }

        override fun onScale(detector: ScaleGestureDetector): Boolean {
            pendingZoom.getAndUpdate { it * detector.scaleFactor }
            pendingFocusX.set(detector.focusX)
            pendingFocusY.set(detector.focusY)
            drawImmediately()
            return true
        }

        override fun onScaleEnd(detector: ScaleGestureDetector) {
            super.onScaleEnd(detector)
        }
    }

    val tapListener = object : GestureDetector.SimpleOnGestureListener() {
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

    fun startRendering() {

        renderJob?.cancel()

        renderJob = renderScope.launch {
            while (isActive) {
                val delayTime = drawWorkspace()

                select<Unit> {
                    redrawChannel.onReceive {
                    }
                    onTimeout(delayTime) {
                    }
                }
            }
        }
    }

    fun stopRendering() {
        renderJob?.cancel()
        nativeLock.withLock { }
    }

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
            if (model.currentTab.value?.type?.isTextEdit() ?: true) {
                wrapperView?.requestFocus()
            }
        }

        return true
    }

    override fun onHoverEvent(event: MotionEvent): Boolean {
        if (event.getToolType(0) == MotionEvent.TOOL_TYPE_STYLUS) {
            val density = context.resources.displayMetrics.density

            when (event.actionMasked) {
                MotionEvent.ACTION_HOVER_MOVE -> {
                    Workspace.mouseMoved(
                        WGPU_OBJ,
                        event.getX(event.actionIndex) / density,
                        event.getY(event.actionIndex) / density,
                    )
                }
            }
            return true
        }
        return super.onHoverEvent(event)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {

        surface = holder.surface

        WGPU_OBJ = Long.MAX_VALUE

        WGPU_OBJ = Workspace.initWS(
            surface!!,
            Lb.lb,
            (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES,
        )

        setWillNotDraw(false)

        isFocusable = true
        isFocusableInTouchMode = true

        requestFocus()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {

        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        stopRendering()

        nativeLock.withLock {
            Workspace.resizeWS(
                WGPU_OBJ,
                holder.surface,
                context.resources.displayMetrics.scaledDensity
            )
        }

        startRendering()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        stopRendering()
    }

    override fun onDetachedFromWindow() {
        renderScope.cancel()

        super.onDetachedFromWindow()
    }

    fun setBottomInset(inset: Int) {
        if (WGPU_OBJ != Long.MAX_VALUE && surface != null) {
            Workspace.setBottomInset(
                WGPU_OBJ,
                inset,
            )
        }
        drawImmediately()
    }

    private suspend fun drawWorkspace(): Long {
        val responseJson = nativeLock.withLock {
            if (WGPU_OBJ == Long.MAX_VALUE || surface == null || surface?.isValid != true) {
                return 0
            }

            val dx = pendingDx.getAndSet(0f)
            val dy = pendingDy.getAndSet(0f)
            val zoom = pendingZoom.getAndSet(1f)
            val focusX = pendingFocusX.getAndSet(0f)
            val focusY = pendingFocusY.getAndSet(0f)

            if (dx != 0f || dy != 0f || zoom != 1f) {
                Workspace.multiTouch(
                    WGPU_OBJ,
                    dx,
                    dy,
                    zoom, focusX, focusY,
                    gestureStartPositions.map { it.x }.toFloatArray(),
                    gestureStartPositions.map { it.y }.toFloatArray()
                )
            }

            // Guard again right before the native call
            if (surface?.isValid != true) {
                return 0
            }

            Workspace.enterFrame(WGPU_OBJ)
        }

        val response: AndroidResponse = frameOutputJsonParser.decodeFromString(responseJson)

        withContext(Dispatchers.Main) {
            if (response.urlOpened.isNotEmpty()) {
                val browserIntent = Intent(Intent.ACTION_VIEW, response.urlOpened.toUri())
                startActivity(context, browserIntent, null)
            }

            val elapsed = System.currentTimeMillis() - model.lastSyncStatusUpdate
            if (elapsed > 1_000) {
                val status: WsStatus = frameOutputJsonParser.decodeFromString(Workspace.getStatus(WGPU_OBJ))
                if (model.isSyncing && !status.syncing) {
                    model._syncCompleted.postValue(Unit)
                }

                model.isSyncing = status.syncing
                model._msg.value = status.msg
                model.lastSyncStatusUpdate = System.currentTimeMillis()
            }
            if (response.newFolderBtnPressed) {
                model._newFolderBtnPressed.postValue(Unit)
            }

            if (response.refreshFiles) {
                model._refreshFiles.postValue(Unit)
            }

            if (!response.docCreated.isNullUUID()) {
                model._createFile.postValue(response.docCreated)
            }

            if (response.tabTitleClicked) {
                model._tabTitleClicked.postValue(Unit)
                Workspace.unfocusTitle(WGPU_OBJ)
            }

            if (response.copiedText.isNotEmpty()) {
                (App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                    .setPrimaryClip(ClipData.newPlainText("", response.copiedText))
            }

            if (response.tabsChanged) {
                val tab = WorkspaceTabType.fromInt(Workspace.currentTab(WGPU_OBJ))

                if (tab != null) {
                    model._currentTab.value = model.currentTab.value?.copy(type = tab)
                }
            }

            if (!response.selectedFile.isNullUUID()) {
                model._currentTab.value = model.currentTab.value?.copy(id = response.selectedFile)
            }

            if (model.currentTab.value?.type == WorkspaceTabType.Markdown) {
                (wrapperView as? WorkspaceTextInputWrapper)?.let { textInputWrapper ->

                    if (response.textUpdated && contextMenu != null) {
                        contextMenu?.finish()
                    }

                    if (response.selectionUpdated) {
                        textInputWrapper.wsInputConnection.notifySelectionUpdated()
                    }

                    if (response.hasEditMenu && contextMenu == null) {
                        val actionModeCallback =
                            TextEditorContextMenu(textInputWrapper)

                        contextMenu = this@WorkspaceView.startActionMode(
                            FloatingTextEditorContextMenu(
                                actionModeCallback,
                                response.editMenuX,
                                response.editMenuY
                            ),
                            ActionMode.TYPE_FLOATING
                        )
                    }
                }
            }
        }

//        return min(response.redrawIn, 500u).toLong()
        return response.redrawIn.toLong()
    }

    fun drawImmediately() {
        redrawChannel.trySend(Unit)
    }

    fun cancelTouches(event: MotionEvent) {
        for (i in 0 until event.pointerCount) {
            val pointerId = event.getPointerId(i)
            val pressure = event.getPressure(i)
            Workspace.touchesCancelled(
                WGPU_OBJ,
                pointerId,
                event.getX(i),
                event.getY(i),
                pressure
            )
        }
    }

    fun forwardedTouchEvent(event: MotionEvent, touchOffsetY: Float) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
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
                    WGPU_OBJ,
                    pointerId,
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure
                )
            }
            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) {
                    val pointerId = event.getPointerId(i)
                    Workspace.touchesMoved(
                        WGPU_OBJ,
                        pointerId,
                        event.getX(i),
                        event.getY(i) + touchOffsetY,
                        getEventPressure(event, i)
                    )
                }
                motionEventPredictor.predict()?.let { predicted ->
                    val density = resources.displayMetrics.density
                    for (i in 0 until predicted.pointerCount) {
                        Workspace.touchesPredicted(
                            WGPU_OBJ,
                            predicted.getPointerId(i),
                            predicted.getX(i) / density,
                            predicted.getY(i) / density + touchOffsetY,
                            getEventPressure(predicted, i)
                        )
                    }
                    predicted.recycle()
                }
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                val pointerId = event.getPointerId(actionIndex)
                Workspace.touchesEnded(
                    WGPU_OBJ,
                    pointerId,
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure
                )
            }
            MotionEvent.ACTION_CANCEL -> {
                val pointerId = event.getPointerId(actionIndex)
                Workspace.touchesCancelled(
                    WGPU_OBJ,
                    pointerId,
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure
                )
            }
        }

        drawImmediately()
    }

    private fun getEventPressure(event: MotionEvent, actionIndex: Int): Float {
        val touchType = event.getToolType(actionIndex)

        val pressure = if (touchType == MotionEvent.TOOL_TYPE_STYLUS) {
            event.pressure * 10f // hack: on the z-fold the range is 0-0.1, uplift this to 0-1
        } else {
            Float.NaN
        }
        return pressure
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        drawImmediately()
    }

    fun openDoc(id: String, newFile: Boolean): WorkspaceTabType? {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return null
        }

        val tab = Workspace.openDoc(WGPU_OBJ, id, newFile)

        return WorkspaceTabType.fromInt(tab)
    }

    fun showTabs(show: Boolean) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.showTabs(WGPU_OBJ, show)
    }

    fun isPenOnlyDraw(): Boolean {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return false
        }

        return Workspace.isPenOnlyDraw(WGPU_OBJ)
    }

    fun getTabs(): Array<String> {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return emptyArray()
        }

        return Workspace.getTabs(WGPU_OBJ)
    }

    fun sync() {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.requestSync(WGPU_OBJ)
    }

    fun closeDoc(id: String) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.closeDoc(WGPU_OBJ, id)
    }

    fun closeAllTabs() {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.closeAllTabs(WGPU_OBJ)
    }

    fun fileRenamed(id: String, name: String) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.fileRenamed(WGPU_OBJ, id, name)
    }

    companion object {
        var WGPU_OBJ = Long.MAX_VALUE

        const val SPEN_ACTION_DOWN = 211
        const val SPEN_ACTION_MOVE = 213
        const val SPEN_ACTION_UP = 212
    }
    inner class FloatingTextEditorContextMenu(private val textEditorContextMenu: TextEditorContextMenu, val editMenuX: Float, val editMenuY: Float) : ActionMode.Callback2() {
        override fun onCreateActionMode(mode: ActionMode?, menu: Menu?): Boolean {
            return textEditorContextMenu.onCreateActionMode(mode, menu)
        }

        override fun onPrepareActionMode(mode: ActionMode?, menu: Menu?): Boolean {
            return textEditorContextMenu.onPrepareActionMode(mode, menu)
        }

        override fun onActionItemClicked(mode: ActionMode?, item: MenuItem?): Boolean {
            return textEditorContextMenu.onActionItemClicked(mode, item)
        }

        override fun onDestroyActionMode(mode: ActionMode?) {
            return textEditorContextMenu.onDestroyActionMode(mode)
        }

        override fun onGetContentRect(mode: ActionMode?, view: View?, outRect: Rect?) {
            outRect!!.set(Rect((editMenuX * context.resources.displayMetrics.scaledDensity).toInt(), (editMenuY * context.resources.displayMetrics.scaledDensity).toInt(), (editMenuX * context.resources.displayMetrics.scaledDensity).toInt(), (editMenuY * context.resources.displayMetrics.scaledDensity).toInt()))
        }
    }

    inner class TextEditorContextMenu(private val textInputWrapper: WorkspaceTextInputWrapper) : ActionMode.Callback {
        override fun onCreateActionMode(mode: ActionMode?, menu: Menu?): Boolean {
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
            if (!textInputWrapper.wsInputConnection.wsEditable.getSelection().isEmpty()) {
                menu.add(Menu.NONE, android.R.id.cut, 0, "Cut")
                    .setAlphabeticShortcut('x')
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)

                menu.add(Menu.NONE, android.R.id.copy, 1, "Copy")
                    .setAlphabeticShortcut('c')
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            }

            menu.add(Menu.NONE, android.R.id.paste, 2, "Paste")
                .setAlphabeticShortcut('v')
                .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)

            menu.add(Menu.NONE, android.R.id.selectAll, 3, "Select all")
                .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
        }

        override fun onPrepareActionMode(mode: ActionMode?, menu: Menu?): Boolean {
            return true
        }

        override fun onActionItemClicked(mode: ActionMode?, item: MenuItem?): Boolean {
            if (item != null) {
                textInputWrapper.wsInputConnection.performContextMenuAction(item.itemId)
            }

            contextMenu!!.finish()

            return true
        }

        override fun onDestroyActionMode(mode: ActionMode?) {
            contextMenu = null
        }
    }
}
