package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.PixelFormat
import android.graphics.Rect
import android.view.ActionMode
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import androidx.core.content.ContextCompat.startActivity
import androidx.core.net.toUri
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
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

@SuppressLint("ViewConstructor")
class WorkspaceView(context: Context, val model: WorkspaceViewModel) : SurfaceView(context), SurfaceHolder.Callback2 {
    private var eraserToggledOnByPen = false

    private var surface: Surface? = null
    var wrapperView: View? = null
    var contextMenu: ActionMode? = null

    private val renderScope = CoroutineScope(Dispatchers.Default + SupervisorJob())
    private var renderJob: Job? = null

    private val nativeLock = ReentrantLock()

    var ignoreSelectionUpdate = AtomicBoolean(false)

    private val redrawChannel = Channel<Unit>(Channel.CONFLATED)
    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    init {
        holder.addCallback(this)
        holder.setFormat(PixelFormat.TRANSPARENT)
    }

    fun startRendering() {

        renderJob?.cancel()

        renderJob = renderScope.launch {
            while (isActive) {
                val delayTime = drawWorkspace()

                select<Unit> {
                    redrawChannel.onReceive {
                        ignoreSelectionUpdate.set(true)
                    }
                    onTimeout(delayTime) {
                        ignoreSelectionUpdate.set(false)
                    }
                }
            }
        }
    }

    fun stopRendering() {
        renderJob?.cancel()
        nativeLock.withLock { }
    }

    private fun adjustTouchPoint(axis: Float): Float {
        return axis / context.resources.displayMetrics.scaledDensity
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            requestFocus()

            forwardedTouchEvent(event, 0f)

            // if they tap outside the toolbar, we want to refocus the text editor to regain text input
            if (model.currentTab.value?.type?.isTextEdit() ?: true) {
                wrapperView?.requestFocus()
            }
        }

        return true
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
        println("draw immedate: ${ignoreSelectionUpdate.get()}")
        val responseJson = nativeLock.withLock {
            if (WGPU_OBJ == Long.MAX_VALUE || surface == null || surface?.isValid != true) {
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
//                    if (response.selectionUpdated && !ignoreSelectionUpdate.get()) {
//                        textInputWrapper.wsInputConnection.notifySelectionUpdated()
//                    }

                    if (response.textUpdated && contextMenu != null) {
                        contextMenu?.finish()
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
        println("draw immediate")
        redrawChannel.trySend(Unit)
    }

    fun forwardedTouchEvent(event: MotionEvent, touchOffsetY: Float) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        val action = event.action and MotionEvent.ACTION_MASK

        for (i in 0 until event.pointerCount) {
            val pointerId = event.getPointerId(i)

            when (action) {
                MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN, SPEN_ACTION_DOWN -> {
                    if (contextMenu != null) {
                        contextMenu!!.finish()
                    }

                    if (action == SPEN_ACTION_DOWN) {
                        eraserToggledOnByPen = true
                        Workspace.toggleEraserSVG(WGPU_OBJ, true)
                    } else if (eraserToggledOnByPen) {
                        eraserToggledOnByPen = false
                        Workspace.toggleEraserSVG(WGPU_OBJ, false)
                    }

                    Workspace.touchesBegin(
                        WGPU_OBJ,
                        pointerId,
                        adjustTouchPoint(event.getX(i)),
                        adjustTouchPoint(event.getY(i) + touchOffsetY),
                        0.0f
                    )
                }

                MotionEvent.ACTION_MOVE, SPEN_ACTION_MOVE -> {
                    Workspace.touchesMoved(
                        WGPU_OBJ,
                        pointerId,
                        adjustTouchPoint(event.getX(i)),
                        adjustTouchPoint(event.getY(i) + touchOffsetY),
                        0.0f
                    )
                }

                MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP, SPEN_ACTION_UP -> {
                    Workspace.touchesEnded(
                        WGPU_OBJ,
                        pointerId,
                        adjustTouchPoint(event.getX(i)),
                        adjustTouchPoint(event.getY(i) + touchOffsetY),
                        0.0f
                    )
                }

                MotionEvent.ACTION_CANCEL -> {
                    Workspace.touchesCancelled(
                        WGPU_OBJ,
                        pointerId,
                        adjustTouchPoint(event.getX(i)),
                        adjustTouchPoint(event.getY(i) + touchOffsetY),
                        0.0f
                    )
                }
            }
        }

        drawImmediately()
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
