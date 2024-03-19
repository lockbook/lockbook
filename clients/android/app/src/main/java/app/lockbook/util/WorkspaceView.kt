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
import androidx.core.view.isVisible
import app.lockbook.model.CoreModel
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.workspace.IntegrationOutput
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.isNullUUID
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import timber.log.Timber
import java.math.BigInteger

// Maybe GLSurfaceView can provide performance improvements?
class WorkspaceView : SurfaceView, SurfaceHolder.Callback2 {
    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    private var redrawTask: Runnable = Runnable {
        invalidate()
    }

    var stateModel: WorkspaceViewModel? = null

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
        if (WGPU_OBJ == Long.MAX_VALUE) {
            return true
        }

        if (event != null) {
            requestFocus()

            for(ptrIndex in 0..(event.pointerCount - 1)) {
                when (event.action) {
                    MotionEvent.ACTION_DOWN -> {
                        WORKSPACE.touchesBegin(
                            WGPU_OBJ,
                            event.getPointerId(ptrIndex),
                            adjustTouchPoint(event.x),
                            adjustTouchPoint(event.y),
                            event.pressure
                        )
                    }

                    MotionEvent.ACTION_MOVE -> {
                        WORKSPACE.touchesMoved(
                            WGPU_OBJ,
                            event.getPointerId(ptrIndex),
                            adjustTouchPoint(event.x),
                            adjustTouchPoint(event.y),
                            event.pressure
                        )
                    }

                    MotionEvent.ACTION_UP -> {
                        WORKSPACE.touchesEnded(
                            WGPU_OBJ,
                            event.getPointerId(ptrIndex),
                            adjustTouchPoint(event.x),
                            adjustTouchPoint(event.y),
                            event.pressure
                        )
                    }
                }
            }

            invalidate()
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

            WORKSPACE.showTabs(WGPU_OBJ, false)

            setWillNotDraw(false)
        }

        isFocusable = true
        isFocusableInTouchMode = true

        requestFocus()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        WS_OBJ = WORKSPACE.dropWgpuCanvas(WGPU_OBJ)
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        invalidate()
    }

    fun openDoc(id: String, newFile: Boolean) {
        Timber.e("about to open doc ${id}...")
        WORKSPACE.openDoc(WGPU_OBJ, id, newFile)
        Timber.e("finished opening doc")
    }

    fun sync() {
        Timber.e("about to sync...")
        WORKSPACE.requestSync(WGPU_OBJ)
        Timber.e("finished sync")
    }

    fun closeDoc(id: String) {
        Timber.e("about to close doc ${id}...")
        WORKSPACE.closeDoc(WGPU_OBJ, id)
        Timber.e("finished opening doc")
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)

        if (WGPU_OBJ == Long.MAX_VALUE) {
            return
        }

        val responseJson = WORKSPACE.enterFrame(WGPU_OBJ)
        val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)
//        Timber.e("got response: ${response}")

        if(response.urlOpened.isNotEmpty()) {
            val browserIntent = Intent(Intent.ACTION_VIEW, Uri.parse(response.urlOpened))
            startActivity(context, browserIntent, null)
        }

        if(stateModel!!.isSyncing && !response.workspaceResp.syncing) {
            stateModel!!._syncCompleted.postValue(Unit)
        }
        stateModel!!.isSyncing = response.workspaceResp.syncing

        if(response.workspaceResp.newFolderBtnPressed) {
            stateModel!!._newFolderBtnPressed.postValue(Unit)
        }

        if(response.workspaceResp.refreshFiles) {
            stateModel!!._refreshFiles.postValue(Unit)
        }

        if(!response.workspaceResp.docCreated.isNullUUID()) {
            stateModel!!._docCreated.postValue(response.workspaceResp.docCreated)
        }

        if(!response.workspaceResp.selectedFile.isNullUUID()) {
            stateModel!!._selectedFile.postValue(response.workspaceResp.selectedFile)
        }

        if(response.workspaceResp.tabTitleClicked) {
            stateModel!!._tabTitleClicked.postValue(Unit)
        }

        stateModel!!._msg.value = response.workspaceResp.msg

        if (response.redrawIn < BigInteger("100")) {
            invalidate()
        } else {
            this.isShown
            handler.postDelayed(redrawTask, response.redrawIn.toLong())
        }

        Timber.e("finished handling response")
    }

    companion object {

        // TODO: move these to the workspace class and make a getInstance for it
        var WGPU_OBJ = Long.MAX_VALUE
        private var WS_OBJ = Long.MAX_VALUE

        val WORKSPACE = Workspace()
    }
}
