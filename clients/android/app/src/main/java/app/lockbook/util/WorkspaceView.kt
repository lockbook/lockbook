package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.net.Uri
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.core.content.ContextCompat.startActivity
import app.lockbook.model.CoreModel
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.workspace.IntegrationOutput
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.isNullUUID
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import timber.log.Timber
import java.lang.Long.max
import java.math.BigInteger

// Maybe GLSurfaceView can provide performance improvements?
class WorkspaceView : SurfaceView, SurfaceHolder.Callback2 {
    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    private var redrawTask: Runnable = Runnable {
        invalidate()
    }

    var model: WorkspaceViewModel? = null

    constructor(context: Context) : super(context)
    constructor(context: Context, attrs: AttributeSet) : super(context, attrs)
    constructor(context: Context, attrs: AttributeSet, defStyle: Int) : super(
        context,
        attrs,
        defStyle
    )

    init {
        holder.addCallback(this)
        this.setZOrderOnTop(true)
        holder.setFormat(PixelFormat.TRANSPARENT)
    }

    private fun adjustTouchPoint(axis: Float): Float {
        return axis / context.resources.displayMetrics.scaledDensity
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if(event != null) {
            requestFocus()
            forwardedTouchEvent(event, 0)
        }

        return true
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (WGPU_OBJ == Long.MAX_VALUE) {
            return
        }

        WORKSPACE.resizeEditor(WGPU_OBJ, holder.surface, context.resources.displayMetrics.scaledDensity)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        holder.let { h ->
            WGPU_OBJ = WORKSPACE.createWgpuCanvas(
                h.surface,
                CoreModel.getPtr(),
                context.resources.displayMetrics.scaledDensity,
                (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES,
                WS_OBJ
            )

            model!!._shouldShowTabs.postValue(Unit)

            setWillNotDraw(false)
        }

        isFocusable = true
        isFocusableInTouchMode = true

        requestFocus()
    }

    fun forwardedTouchEvent(event: MotionEvent, touchOffsetY: Int) {
        if (WGPU_OBJ == Long.MAX_VALUE) {
            return
        }

        if(event.pointerCount > 0) {
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    WORKSPACE.touchesBegin(
                        WGPU_OBJ,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y + touchOffsetY),
                        event.pressure
                    )
                }

                MotionEvent.ACTION_MOVE -> {
                    WORKSPACE.touchesMoved(
                        WGPU_OBJ,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y + touchOffsetY),
                        event.pressure
                    )
                }

                MotionEvent.ACTION_UP -> {
                    WORKSPACE.touchesEnded(
                        WGPU_OBJ,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y + touchOffsetY),
                        event.pressure
                    )
                }
            }
        }

        invalidate()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        WS_OBJ = WORKSPACE.dropWgpuCanvas(WGPU_OBJ)
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        invalidate()
    }

    fun openDoc(id: String, newFile: Boolean) {
        WORKSPACE.openDoc(WGPU_OBJ, id, newFile)
    }

    fun showTabs(show: Boolean) {
        WORKSPACE.showTabs(WGPU_OBJ, show)
    }

    fun sync() {
        model!!.isSyncing = true
        WORKSPACE.requestSync(WGPU_OBJ)
    }

    fun closeDoc(id: String) {
        WORKSPACE.closeDoc(WGPU_OBJ, id)
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)

        if (WGPU_OBJ == Long.MAX_VALUE) {
            return
        }

        val responseJson = WORKSPACE.enterFrame(WGPU_OBJ)
        val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)

        if(response.urlOpened.isNotEmpty()) {
            val browserIntent = Intent(Intent.ACTION_VIEW, Uri.parse(response.urlOpened))
            startActivity(context, browserIntent, null)
        }

        if(model!!.isSyncing && !response.workspaceResp.syncing) {
            model!!._syncCompleted.postValue(Unit)
        }
        model!!.isSyncing = response.workspaceResp.syncing

        if(response.workspaceResp.newFolderBtnPressed) {
            model!!._newFolderBtnPressed.postValue(Unit)
        }

        if(response.workspaceResp.refreshFiles) {
            model!!._refreshFiles.postValue(Unit)
        }

        if(!response.workspaceResp.docCreated.isNullUUID()) {
            model!!._docCreated.postValue(response.workspaceResp.docCreated)
        }

        if(!response.workspaceResp.selectedFile.isNullUUID()) {
            model!!._selectedFile.postValue(response.workspaceResp.selectedFile)
        }

        if(response.workspaceResp.tabTitleClicked) {
            model!!._tabTitleClicked.postValue(Unit)
        }

        model!!._msg.value = response.workspaceResp.msg
        val currentTab = WorkspaceTab.fromInt(WORKSPACE.currentTab(WGPU_OBJ))
        if(currentTab != model!!._currentTab.value) {
            model!!._currentTab.value = currentTab
        }

        if (response.redrawIn < BigInteger("100")) {
            invalidate()
        } else {
            handler.postDelayed(redrawTask, max(response.redrawIn.toLong(), 500L))
        }
    }

    companion object {
        // TODO: move these to the workspace class and make a getInstance for it
        var WGPU_OBJ = Long.MAX_VALUE
        private var WS_OBJ = Long.MAX_VALUE

        val WORKSPACE = Workspace()
    }
}
