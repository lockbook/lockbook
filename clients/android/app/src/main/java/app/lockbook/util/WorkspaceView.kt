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
import android.os.Build
import android.os.LocaleList
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
import android.widget.Magnifier
import android.widget.OverScroller
import android.widget.Toast
import androidx.core.content.ContextCompat.startActivity
import androidx.core.content.FileProvider
import androidx.core.graphics.Insets
import androidx.core.net.toUri
import androidx.input.motionprediction.MotionEventPredictor
import androidx.preference.PreferenceManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.model.OpenFileRequest
import app.lockbook.model.WorkspaceTabType
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.screen.WorkspaceTextInputWrapper
import app.lockbook.workspace.AndroidResponse
import app.lockbook.workspace.JTextRange
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.isNullUUID
import app.lockbook.workspace.toModelTab
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import net.lockbook.Lb
import java.io.File

@SuppressLint("ViewConstructor", "SoonBlockedPrivateApi")
class WorkspaceView(
    context: Context,
    val model: WorkspaceViewModel,
) : SurfaceView(context),
    SurfaceHolder.Callback2 {
    private var surface: Surface? = null

    // The fragment may request a file before SurfaceView creates its native workspace.
    private var deferredOpenDoc: OpenFileRequest? = null
    private var safeInsets = Insets.NONE
    var wrapperView: View? = null
    var onMarkdownToolbarStateChanged: (() -> Unit)? = null
    var contextMenu: ActionMode? = null
    private var magnifier: Magnifier? = null

    private var redrawTask: Runnable =
        Runnable {
            invalidate()
        }
    private val choreographer: Choreographer by lazy { Choreographer.getInstance() }

    private val prefs by lazy { PreferenceManager.getDefaultSharedPreferences(context) }
    private val contactLinkedSitesKey by lazy { context.getString(R.string.contact_linked_sites_key) }
    private val spellCheckKey by lazy { context.getString(R.string.spell_check_key) }
    private val spelling by lazy { WorkspaceSpelling(context) { invalidate() } }

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
    private var lastScaleFocusX = 0f
    private var lastScaleFocusY = 0f

    private val scaleListener =
        object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScaleBegin(detector: ScaleGestureDetector): Boolean {
                pendingZoom = 1f
                lastScaleFocusX = detector.focusX
                lastScaleFocusY = detector.focusY
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
                pendingDx += detector.focusX - lastScaleFocusX
                pendingDy += detector.focusY - lastScaleFocusY
                pendingZoom *= detector.scaleFactor
                pendingFocusX = detector.focusX
                pendingFocusY = detector.focusY
                lastScaleFocusX = detector.focusX
                lastScaleFocusY = detector.focusY
                return true
            }

            override fun onScaleEnd(detector: ScaleGestureDetector) {
                super.onScaleEnd(detector)
            }
        }

    val tapListener =
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDoubleTap(e: MotionEvent): Boolean {
                touchForwarder.cancelTouches(e)
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
    val touchForwarder =
        WorkspaceTouchForwarder(
            this,
            motionEventPredictor,
        )
    val keyboardPull = KeyboardPull(this)

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            requestFocus()
            motionEventPredictor.record(event)

            scaleDetector.onTouchEvent(event)
            scrollDetector.onTouchEvent(event)
            tapDetector.onTouchEvent(event)

            touchForwarder.forward(event, 0f)

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
        val darkMode =
            (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        val workspaceTheme = WorkspaceThemeHelper.materialTheme(context, darkMode)

        // includes the app's own language choice, which the system property misses
        Workspace.setPreferredLanguages(LocaleList.getDefault().toLanguageTags())
        wgpuObj =
            Workspace.initWSOffloaded(
                surface!!,
                Lb.lb,
                workspaceTheme,
            )
        publishSafeInsets()
        setWillNotDraw(false)

        isFocusable = true
        isFocusableInTouchMode = true

        requestFocus()

        deferredOpenDoc?.let(::openDoc)
        publishOpenTabCount()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)

        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        val darkMode =
            (newConfig.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
        val workspaceTheme = WorkspaceThemeHelper.materialTheme(context, darkMode)
        Workspace.setTheme(wgpuObj, workspaceTheme)
        dismissMagnifier()
        invalidate()
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
        publishSafeInsets()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        handler?.removeCallbacks(redrawTask)
        dismissMagnifier()
        surface = null
    }

    override fun onDetachedFromWindow() {
        handler?.removeCallbacks(redrawTask)
        ioScope.cancel()
        super.onDetachedFromWindow()
    }

    fun setSafeInsets(insets: Insets) {
        if (safeInsets == insets) return
        safeInsets = insets
        publishSafeInsets()
        invalidate()
    }

    private fun publishSafeInsets() {
        if (wgpuObj != Long.MAX_VALUE && surface != null) {
            Workspace.setSafeInsets(wgpuObj, safeInsets.top, safeInsets.right, safeInsets.bottom, safeInsets.left)
        }
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

        Workspace.setContactLinkedSites(wgpuObj, prefs.getBoolean(contactLinkedSitesKey, false))
        Workspace.setSpellCheck(wgpuObj, prefs.getBoolean(spellCheckKey, true))

        model.nextAttachment()?.let { attachment ->
            when (Workspace.currentTab(wgpuObj).toModelTab().type) {
                WorkspaceTabType.Loading -> {
                    // Keep the staged file until the editor is ready.
                }

                WorkspaceTabType.Markdown -> {
                    val accepted = Workspace.queueFileForEditorImport(wgpuObj, attachment.tempPath, attachment.name)
                    model.removeNextAttachment()
                    File(attachment.tempPath).delete()
                    if (!accepted) {
                        Toast.makeText(context, R.string.workspace_attachment_import_failed, Toast.LENGTH_SHORT).show()
                    }
                    invalidate()
                }

                else -> {
                    model.removeNextAttachment()
                    File(attachment.tempPath).delete()
                    Toast.makeText(context, R.string.workspace_attachment_import_failed, Toast.LENGTH_SHORT).show()
                    invalidate()
                }
            }
        }

        // Insets need not change when another document becomes active or finishes loading.
        Workspace.setKeyboardShown(wgpuObj, model.keyboardVisible.value ?: false)

        val response: AndroidResponse = Workspace.enterFrameOffloaded(wgpuObj)
        if (response.failureMessage.isNotEmpty()) {
            Toast.makeText(context, response.failureMessage, Toast.LENGTH_LONG).show()
        }

        val outlineOpen = Workspace.outlineOpen(wgpuObj)
        if (outlineOpen != model.outlineOpen.value) {
            model._outlineOpen.value = outlineOpen
        }

        if (response.urlOpened.isNotEmpty()) {
            try {
                val browserIntent = Intent(Intent.ACTION_VIEW, response.urlOpened.toUri())
                startActivity(context, browserIntent, null)
            } catch (err: Exception) {
                Toast.makeText(context, err.message, Toast.LENGTH_SHORT).show()
            }
        }

        if (response.copiedText.isNotEmpty()) {
            val clip =
                if (response.copiedHtml.isEmpty()) {
                    ClipData.newPlainText("", response.copiedText)
                } else {
                    ClipData.newHtmlText("", response.copiedText, response.copiedHtml)
                }
            (
                App
                    .applicationContext()
                    .getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            ).setPrimaryClip(clip)
        }

        if (response.copiedImage.isNotEmpty()) {
            copyImageToClipboard(response.copiedImage)
        }

        val currentTab =
            if (response.tabsChanged || !response.selectedFile.isNullUUID()) {
                publishOpenTabCount()
                Workspace.currentTab(wgpuObj).toModelTab()
            } else {
                null
            }

        if (currentTab != null) {
            model._currentTab.value = currentTab
        }

        if (
            response.hasEditMenu &&
            response.editMenuForImage &&
            contextMenu == null
        ) {
            contextMenu =
                this@WorkspaceView.startActionMode(
                    FloatingImageContextMenu(
                        response.editMenuX,
                        response.editMenuY,
                    ),
                    ActionMode.TYPE_FLOATING,
                )
        } else if (
            model.currentTab.value?.type == WorkspaceTabType.Markdown ||
            model.currentTab.value?.type == WorkspaceTabType.Chat
        ) {
            (wrapperView as? WorkspaceTextInputWrapper)?.let { textInputWrapper ->
                val density = context.resources.displayMetrics.scaledDensity
                textInputWrapper.applyTextInteractionRect(
                    response.hasTextInteractionRect,
                    response.textInteractionMinX,
                    response.textInteractionMinY,
                    response.textInteractionMaxX,
                    response.textInteractionMaxY,
                    density,
                )

                if (response.textUpdated && contextMenu != null) {
                    contextMenu?.finish()
                }

                if (response.selectionUpdated) {
                    textInputWrapper.wsInputConnection.notifySelectionUpdated()
                } else if (response.textUpdated || response.scrollUpdated) {
                    textInputWrapper.wsInputConnection.onEditorGeometryChanged()
                }

                updateMagnifier(response, density)

                response.virtualKeyboardShown?.let { model._showKeyboard.value = it }
                keyboardPull.selecting = response.hasMagnifier

                if (response.hasEditMenu && contextMenu == null) {
                    val actionModeCallback =
                        TextEditorContextMenu(textInputWrapper, response.editMenuForAtom)

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
        } else {
            dismissMagnifier()
        }

        if (response.selectionUpdated || response.textUpdated) {
            onMarkdownToolbarStateChanged?.invoke()
        }

        spelling.check(wgpuObj)

        if (response.redrawIn < 100) {
            invalidate()
        } else {
            handler.postDelayed(redrawTask, response.redrawIn)
        }
    }

    fun drawImmediately() {
        drawWorkspace()
    }

    private fun copyImageToClipboard(image: ByteArray) {
        val shareDirectory = File(context.cacheDir, "share/clipboard").apply { mkdirs() }
        // a gif copies as itself, else the bytes are a png
        val gif = image.size >= 4 && String(image, 0, 4, Charsets.US_ASCII) == "GIF8"
        val imageFile = File(shareDirectory, if (gif) "image.gif" else "image.png")
        imageFile.writeBytes(image)

        val uri = FileProvider.getUriForFile(context, "app.lockbook.fileprovider", imageFile)
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newUri(context.contentResolver, "Image", uri))
    }

    @Suppress("DEPRECATION")
    private fun updateMagnifier(
        response: AndroidResponse,
        density: Float,
    ) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.P) {
            return
        }
        if (!response.hasMagnifier) {
            magnifier?.dismiss()
            return
        }
        // Deprecated ctor applies system text-magnifier defaults (5% white
        // overlay). `Magnifier.Builder()` leaves overlay null.
        val mag = magnifier ?: Magnifier(this).also { magnifier = it }
        mag.show(response.magnifierX * density, response.magnifierY * density)
        mag.update()
    }

    private fun dismissMagnifier() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            magnifier?.dismiss()
        }
        magnifier = null
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)

        drawWorkspace()
    }

    fun launchIo(block: suspend () -> Unit) {
        ioScope.launch { block() }
    }

    fun canForwardTouches(): Boolean = wgpuObj != Long.MAX_VALUE && surface != null

    fun willConsumeTouches(
        x: Float,
        y: Float,
    ): Boolean {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return false
        }

        return Workspace.willConsumeTouches(wgpuObj, x, y)
    }

    fun setKeyboardShown(shown: Boolean) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.setKeyboardShown(wgpuObj, shown)
        invalidate()
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        drawImmediately()
    }

    fun openDoc(request: OpenFileRequest) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            deferredOpenDoc = request
            return
        }

        deferredOpenDoc = null
        val heading = request.heading
        if (heading.isNullOrEmpty()) {
            Workspace.openDoc(wgpuObj, request.id, request.newFile)
        } else {
            Workspace.openDocAtHeading(wgpuObj, request.id, heading)
        }
        model.recordTabCloseBehavior(Workspace.currentTab(wgpuObj).toModelTab().sessionId, request.closeDetailOnBack)
        publishOpenTabCount()
        invalidate()
    }

    fun toggleSource() {
        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        Workspace.toggleSource(wgpuObj)
        invalidate()
    }

    fun toggleOutline() {
        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        Workspace.toggleOutline(wgpuObj)
        invalidate()
    }

    fun activateDoc(id: String) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.activateDoc(wgpuObj, id)
        publishOpenTabCount()
        invalidate()
    }

    fun closeCurrentTabToWelcome(): Boolean {
        if (wgpuObj == Long.MAX_VALUE || surface == null) return false
        if (!Workspace.closeCurrentTabToWelcome(wgpuObj)) return false
        publishOpenTabCount()
        model._currentTab.value = Workspace.currentTab(wgpuObj).toModelTab()
        invalidate()
        return true
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
        if (model.currentTab.value?.type == WorkspaceTabType.Image) {
            return true
        }

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

    private fun publishOpenTabCount() {
        model.updateOpenTabCount(getTabs().size)
    }

    fun closeDoc(id: String) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.closeDoc(wgpuObj, id)
        publishOpenTabCount()
        invalidate()
    }

    fun closeSession(id: String) {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.closeSession(wgpuObj, id)
        publishOpenTabCount()
        invalidate()
    }

    fun closeAllTabs() {
        if (wgpuObj == Long.MAX_VALUE || surface == null) {
            return
        }

        Workspace.closeAllTabs(wgpuObj)
        publishOpenTabCount()
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

        const val EDIT_ATOM_MENU_ID = 0x00E0170A // arbitrary; distinct from android.R.id.*
        const val OPEN_LINK_MENU_ID = 0x00E0170B
        const val COPY_LINK_MENU_ID = 0x00E0170C
        const val REFRESH_PREVIEW_MENU_ID = 0x00E0170D
        const val COPY_HEADING_LINK_MENU_ID = 0x00E0170F
        const val COPY_RENDERED_MENU_ID = 0x00E0170E
        const val ADD_TO_DICTIONARY_MENU_ID = 0x00E01710
        const val IGNORE_SPELLING_MENU_ID = 0x00E01711
        const val SPELLING_GUESS_GROUP_ID = 0x00E01712
        const val COPY_IMAGE_MENU_ID = 0x00E01713
        const val OPEN_DIAGRAM_MENU_ID = 0x00E01714
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
            if (outRect != null) {
                outRect!!.set(
                    Rect(
                        (editMenuX * context.resources.displayMetrics.scaledDensity).toInt(),
                        (
                            editMenuY *
                                context.resources.displayMetrics.scaledDensity
                        ).toInt(),
                        (editMenuX * context.resources.displayMetrics.scaledDensity).toInt(),
                        (editMenuY * context.resources.displayMetrics.scaledDensity).toInt(),
                    ),
                )
            }
        }
    }

    inner class FloatingImageContextMenu(
        private val menuX: Float,
        private val menuY: Float,
    ) : ActionMode.Callback2() {
        override fun onCreateActionMode(
            mode: ActionMode?,
            menu: Menu?,
        ): Boolean {
            mode?.apply {
                title = null
                subtitle = null
                titleOptionalHint = true
            }
            menu
                ?.add(Menu.NONE, android.R.id.copy, 0, "Copy image")
                ?.setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            return true
        }

        override fun onPrepareActionMode(
            mode: ActionMode?,
            menu: Menu?,
        ): Boolean = true

        override fun onActionItemClicked(
            mode: ActionMode?,
            item: MenuItem?,
        ): Boolean {
            if (item?.itemId != android.R.id.copy) {
                return false
            }

            Workspace.copyImage(wgpuObj)
            invalidate()
            mode?.finish()
            return true
        }

        override fun onDestroyActionMode(mode: ActionMode?) {
            contextMenu = null
        }

        override fun onGetContentRect(
            mode: ActionMode?,
            view: View?,
            outRect: Rect?,
        ) {
            val density = context.resources.displayMetrics.scaledDensity
            val x = (menuX * density).toInt()
            val y = (menuY * density).toInt()
            outRect?.set(x, y, x, y)
        }
    }

    inner class TextEditorContextMenu(
        private val textInputWrapper: WorkspaceTextInputWrapper,
        private val forAtom: Boolean = false,
    ) : ActionMode.Callback {
        // URL under the selection when the menu was built; feeds "Copy Link"
        private var openTarget: String? = null
        private var headingLink: String? = null

        // the misspelled word under the selection
        private var misspelling: JTextRange? = null
        private var misspelled: String? = null

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

        private fun populateSpellingItems(menu: Menu) {
            val range = Workspace.selectionMisspelling(wgpuObj)
            if (range.none) {
                misspelling = null
                return
            }
            val word = Workspace.getTextInRange(wgpuObj, range.start, range.end)
            misspelling = range
            misspelled = word
            spelling.guesses(word).forEachIndexed { i, guess ->
                menu
                    .add(SPELLING_GUESS_GROUP_ID, SPELLING_GUESS_GROUP_ID + 1 + i, 0, guess)
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            }
            menu
                .add(Menu.NONE, ADD_TO_DICTIONARY_MENU_ID, 0, "Add to dictionary")
                .setShowAsAction(MenuItem.SHOW_AS_ACTION_NEVER)
            menu
                .add(Menu.NONE, IGNORE_SPELLING_MENU_ID, 0, "Ignore")
                .setShowAsAction(MenuItem.SHOW_AS_ACTION_NEVER)
        }

        private fun populateMenuWithItems(menu: Menu) {
            populateSpellingItems(menu)

            // link (or wikilink/image) in the selection → offer open + copy
            openTarget = Workspace.selectionOpenTarget(wgpuObj)
            if (openTarget != null) {
                val open = if (Workspace.selectionTargetIsImage(wgpuObj)) "Open Image" else "Open Link"
                menu
                    .add(Menu.NONE, OPEN_LINK_MENU_ID, 0, open)
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)

                menu
                    .add(Menu.NONE, COPY_LINK_MENU_ID, 0, "Copy Link")
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            }

            headingLink = Workspace.selectionHeadingLink(wgpuObj)
            if (headingLink != null) {
                menu
                    .add(Menu.NONE, COPY_HEADING_LINK_MENU_ID, 0, "Copy Heading Link")
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            }

            if (forAtom && Workspace.selectionHasDiagram(wgpuObj)) {
                menu
                    .add(Menu.NONE, OPEN_DIAGRAM_MENU_ID, 0, "Open")
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            }

            if (forAtom) {
                // select the URL inside the atom, revealing its source —
                // the only touch path in, since mobile has no arrow keys
                menu
                    .add(Menu.NONE, EDIT_ATOM_MENU_ID, 0, "Edit")
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)

                if (Workspace.selectionHasPicture(wgpuObj)) {
                    menu
                        .add(Menu.NONE, COPY_IMAGE_MENU_ID, 0, "Copy Image")
                        .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
                }

                if (openTarget != null) {
                    menu
                        .add(Menu.NONE, REFRESH_PREVIEW_MENU_ID, 0, "Refresh Preview")
                        .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
                }
            }

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

                menu
                    .add(Menu.NONE, COPY_RENDERED_MENU_ID, 2, "Copy Rendered")
                    .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)
            }

            menu
                .add(Menu.NONE, android.R.id.paste, 3, "Paste")
                .setAlphabeticShortcut('v')
                .setShowAsAction(MenuItem.SHOW_AS_ACTION_ALWAYS)

            menu
                .add(Menu.NONE, android.R.id.selectAll, 4, "Select all")
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
                val range = misspelling
                val word = misspelled
                when (item.itemId) {
                    in (SPELLING_GUESS_GROUP_ID + 1)..(SPELLING_GUESS_GROUP_ID + WorkspaceSpelling.MAX_GUESSES) -> {
                        // the word may have moved while the menu was open
                        if (range != null && Workspace.getTextInRange(wgpuObj, range.start, range.end) == word) {
                            textInputWrapper.wsInputConnection.wsEditable
                                .replace(range.start, range.end, item.title)
                            invalidate()
                        }
                    }

                    ADD_TO_DICTIONARY_MENU_ID -> {
                        if (word != null) {
                            Workspace.spellingAnswer(wgpuObj, word, true)
                            context.startActivity(
                                android.content
                                    .Intent("com.android.settings.USER_DICTIONARY_INSERT")
                                    .putExtra("word", word)
                                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK),
                            )
                            invalidate()
                        }
                    }

                    IGNORE_SPELLING_MENU_ID -> {
                        if (word != null) {
                            Workspace.spellingIgnore(wgpuObj, word)
                            invalidate()
                        }
                    }

                    EDIT_ATOM_MENU_ID -> {
                        Workspace.enterSelectedAtom(wgpuObj)
                        // schedule the frame that processes the event, whose selection
                        // update then flows back via `response.selectionUpdated`
                        invalidate()
                    }

                    OPEN_LINK_MENU_ID -> {
                        Workspace.openSelectionLinks(wgpuObj)
                        // schedule the frame that processes the open, which flows
                        // back via `response.urlOpened` / `response.selectedFile`
                        invalidate()
                    }

                    REFRESH_PREVIEW_MENU_ID -> {
                        Workspace.refreshSelectionPreviews(wgpuObj)
                        invalidate()
                    }

                    COPY_HEADING_LINK_MENU_ID -> {
                        headingLink?.let {
                            (
                                context.getSystemService(Context.CLIPBOARD_SERVICE)
                                    as ClipboardManager
                            ).setPrimaryClip(ClipData.newPlainText("", it))
                        }
                    }

                    OPEN_DIAGRAM_MENU_ID -> {
                        Workspace.openSelectedDiagram(wgpuObj)
                        invalidate()
                    }

                    COPY_IMAGE_MENU_ID -> {
                        Workspace.copyImage(wgpuObj)
                        // the frame that processes the copy fills `response.copiedImage`
                        invalidate()
                    }

                    COPY_RENDERED_MENU_ID -> {
                        Workspace.copyRendered(wgpuObj)
                        // the frame that processes the copy fills `response.copiedHtml`
                        invalidate()
                    }

                    COPY_LINK_MENU_ID -> {
                        openTarget?.let {
                            (
                                context.getSystemService(Context.CLIPBOARD_SERVICE)
                                    as ClipboardManager
                            ).setPrimaryClip(ClipData.newPlainText("", it))
                        }
                    }

                    else -> {
                        textInputWrapper.wsInputConnection.performContextMenuAction(item.itemId)
                    }
                }
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
