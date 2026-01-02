package app.lockbook.util

import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Rect
import android.view.ActionMode
import android.view.Choreographer
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.widget.FrameLayout
import androidx.core.content.ContextCompat.startActivity
import androidx.core.net.toUri
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import app.lockbook.App
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.screen.WorkspaceTextInputWrapper
import app.lockbook.workspace.AndroidResponse
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.WsStatus
import app.lockbook.workspace.isNullUUID
import com.google.android.material.color.MaterialColors
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import net.lockbook.Lb
import kotlin.math.min

@SuppressLint("ViewConstructor")
class WorkspaceView(context: Context, val model: WorkspaceViewModel) : SurfaceView(context), SurfaceHolder.Callback2 {
    private var eraserToggledOnByPen = false

    private var surface: Surface? = null
    var wrapperView: View? = null
    var contextMenu: ActionMode? = null

    var ignoreSelectionUpdate = false

    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }



    private var redrawTask: Runnable = Runnable {
        invalidate()
    }

    init {
        holder.addCallback(this)
        holder.setFormat(PixelFormat.TRANSLUCENT)
    }

    private fun adjustTouchPoint(axis: Float): Float {
        return axis / context.resources.displayMetrics.scaledDensity
    }

    fun setBottomInset(inset: Int) {
        if (WGPU_OBJ != Long.MAX_VALUE && surface != null) {
            Workspace.setBottomInset(
                WGPU_OBJ,
                inset,
            )
        }
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            requestFocus()

            forwardedTouchEvent(event, 0f)

            // if they tap outside the toolbar, we want to refocus the text editor to regain text input
            if (model.currentTab.value == WorkspaceTab.Markdown || model.currentTab.value == WorkspaceTab.PlainText) {
                wrapperView?.requestFocus()
            }
        }

        return true
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }
//        Workspace.resizeWS(
//            WGPU_OBJ,
//            width,
//            height,
//            context.resources.displayMetrics.scaledDensity
//        )


    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        surface = holder.surface
        println("3asba: surface created")
        WGPU_OBJ = Workspace.initWS(
            surface!!,
            Lb.lb,
            context.resources.displayMetrics.scaledDensity,
            (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES,
            WGPU_OBJ
        )

        model._shouldShowTabs.postValue(Unit)

        setWillNotDraw(false)

//        holder.setFixedSize(context.resources.displayMetrics.widthPixels, context.resources.displayMetrics.heightPixels)

        isFocusable = true
        isFocusableInTouchMode = true

        requestFocus()
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

        invalidate()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        surface = null
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        invalidate()
    }

    fun openDoc(id: String, newFile: Boolean) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.openDoc(WGPU_OBJ, id, newFile)
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

    fun fileRenamed(id: String, name: String) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.fileRenamed(WGPU_OBJ, id, name)
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)
        drawWorkspace()
    }

    fun drawImmediately() {
        ignoreSelectionUpdate = true
        drawWorkspace()
        ignoreSelectionUpdate = false
    }

    private fun drawWorkspace() {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null || surface?.isValid != true) {
            return
        }

        val responseJson = Workspace.enterFrame(WGPU_OBJ)
        val response: AndroidResponse = frameOutputJsonParser.decodeFromString(responseJson)

        if (response.urlOpened.isNotEmpty()) {
            val browserIntent = Intent(Intent.ACTION_VIEW, response.urlOpened.toUri())
            startActivity(context, browserIntent, null)
        }

        val elapsed = System.currentTimeMillis() - model.lastSyncStatusUpdate
        // refresh every second
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
            model._docCreated.postValue(response.docCreated)
        }

        if (!response.selectedFile.isNullUUID()) {
            model._selectedFile.value = response.selectedFile
        }

        if (response.tabTitleClicked) {
            model._tabTitleClicked.postValue(Unit)
            Workspace.unfocusTitle(WGPU_OBJ)
        }

        if (response.copiedText.isNotEmpty()) {
            (App.applicationContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                .setPrimaryClip(ClipData.newPlainText("", response.copiedText))
        }

        val currentTab = WorkspaceTab.fromInt(Workspace.currentTab(WGPU_OBJ))
        if (response.tabsChanged) {
            model._currentTab.value = currentTab
        }

        if (currentTab == WorkspaceTab.Markdown) {
            (wrapperView as? WorkspaceTextInputWrapper)?.let { textInputWrapper ->
                if (response.selectionUpdated && !ignoreSelectionUpdate) {
                    textInputWrapper.wsInputConnection.notifySelectionUpdated()
                }

                if (response.textUpdated && contextMenu != null) {
                    contextMenu?.finish()
                }

                if (response.hasEditMenu && contextMenu == null) {
                    val actionModeCallback =
                        TextEditorContextMenu(textInputWrapper)

                    contextMenu = this.startActionMode(
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

        if (response.redrawIn < 100u) {
            invalidate()
        } else {
            handler.postDelayed(redrawTask, min(response.redrawIn, 500u).toLong())
        }
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

