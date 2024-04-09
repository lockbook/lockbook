package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.net.Uri
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import androidx.core.content.ContextCompat.startActivity
import app.lockbook.model.CoreModel
import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceViewModel
import app.lockbook.workspace.IntegrationOutput
import app.lockbook.workspace.Workspace
import app.lockbook.workspace.isNullUUID
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import java.lang.Long.max
import java.math.BigInteger

@SuppressLint("ViewConstructor")
class WorkspaceView(context: Context, val model: WorkspaceViewModel) : SurfaceView(context), SurfaceHolder.Callback2 {
    private var eraserToggledOnByPen = false

    private var surface: Surface? = null
    var wrapperView: View? = null

    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    private var redrawTask: Runnable = Runnable {
        invalidate()
    }

    init {
        holder.addCallback(this)
        holder.setFormat(PixelFormat.TRANSPARENT)
    }

    private fun adjustTouchPoint(axis: Float): Float {
        return axis / context.resources.displayMetrics.scaledDensity
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent?): Boolean {
        if (event != null) {
            requestFocus()

            forwardedTouchEvent(event, 0)

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

        WORKSPACE.resizeEditor(WGPU_OBJ, holder.surface, context.resources.displayMetrics.scaledDensity)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        surface = holder.surface

        WGPU_OBJ = WORKSPACE.initWS(
            surface!!,
            CoreModel.getPtr(),
            context.resources.displayMetrics.scaledDensity,
            (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES,
            WGPU_OBJ
        )

        model._shouldShowTabs.postValue(Unit)

        setWillNotDraw(false)

        isFocusable = true
        isFocusableInTouchMode = true

        requestFocus()
    }

    fun forwardedTouchEvent(event: MotionEvent, touchOffsetY: Int) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        val action = event.action and MotionEvent.ACTION_MASK

        for (i in 0 until event.pointerCount) {
            val pointerId = event.getPointerId(i)

            when (action) {
                MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN, SPEN_ACTION_DOWN -> {
                    if (action == SPEN_ACTION_DOWN) {
                        eraserToggledOnByPen = true
                        WORKSPACE.toggleEraserSVG(WGPU_OBJ, true)
                    } else if (eraserToggledOnByPen) {
                        eraserToggledOnByPen = false
                        WORKSPACE.toggleEraserSVG(WGPU_OBJ, false)
                    }

                    WORKSPACE.touchesBegin(
                        WGPU_OBJ,
                        pointerId,
                        adjustTouchPoint(event.getX(i)),
                        adjustTouchPoint(event.getY(i) + touchOffsetY),
                        0.0f
                    )
                }
                MotionEvent.ACTION_MOVE, SPEN_ACTION_MOVE -> {
                    WORKSPACE.touchesMoved(
                        WGPU_OBJ,
                        pointerId,
                        adjustTouchPoint(event.getX(i)),
                        adjustTouchPoint(event.getY(i) + touchOffsetY),
                        0.0f
                    )
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP, SPEN_ACTION_UP -> {
                    WORKSPACE.touchesEnded(
                        WGPU_OBJ,
                        pointerId,
                        adjustTouchPoint(event.getX(i)),
                        adjustTouchPoint(event.getY(i) + touchOffsetY),
                        0.0f
                    )
                }
                MotionEvent.ACTION_CANCEL -> {
                    WORKSPACE.touchesCancelled(
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

        WORKSPACE.openDoc(WGPU_OBJ, id, newFile)
    }

    fun showTabs(show: Boolean) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        WORKSPACE.showTabs(WGPU_OBJ, show)
    }

    fun sync() {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        WORKSPACE.requestSync(WGPU_OBJ)
    }

    fun closeDoc(id: String) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        WORKSPACE.closeDoc(WGPU_OBJ, id)
    }

    fun fileRenamed(id: String, name: String) {
        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        WORKSPACE.fileRenamed(WGPU_OBJ, id, name)
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)

        if (WGPU_OBJ == Long.MAX_VALUE || surface == null) {
            return
        }

        val responseJson = WORKSPACE.enterFrame(WGPU_OBJ)
        val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)

        if (response.urlOpened.isNotEmpty()) {
            val browserIntent = Intent(Intent.ACTION_VIEW, Uri.parse(response.urlOpened))
            startActivity(context, browserIntent, null)
        }

        if (model.isSyncing && !response.workspaceResp.syncing) {
            model._syncCompleted.postValue(Unit)
        }
        model.isSyncing = response.workspaceResp.syncing

        if (response.workspaceResp.newFolderBtnPressed) {
            model._newFolderBtnPressed.postValue(Unit)
        }

        if (response.workspaceResp.refreshFiles) {
            model._refreshFiles.postValue(Unit)
        }

        if (!response.workspaceResp.docCreated.isNullUUID()) {
            model._docCreated.postValue(response.workspaceResp.docCreated)
        }

        if (!response.workspaceResp.selectedFile.isNullUUID()) {
            model._selectedFile.value = response.workspaceResp.selectedFile
        }

        if (response.workspaceResp.tabTitleClicked) {
            model._tabTitleClicked.postValue(Unit)
            WORKSPACE.unfocusTitle(WGPU_OBJ)
        }

        model._msg.value = response.workspaceResp.msg

        val currentTab = WorkspaceTab.fromInt(WORKSPACE.currentTab(WGPU_OBJ))
        if (currentTab != model._currentTab.value) {
            model._currentTab.value = currentTab
        }

        if (response.redrawIn < BigInteger("100")) {
            invalidate()
        } else {
            handler.postDelayed(redrawTask, max(response.redrawIn.toLong(), 500L))
        }
    }

    companion object {
        var WGPU_OBJ = Long.MAX_VALUE

        const val SPEN_ACTION_DOWN = 211
        const val SPEN_ACTION_MOVE = 213
        const val SPEN_ACTION_UP = 212

        val WORKSPACE = Workspace.getInstance()
    }
}
