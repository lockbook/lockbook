package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import app.lockbook.model.CoreModel
import app.lockbook.model.TextEditorViewModel
import app.lockbook.workspace.IntegrationOutput
import app.lockbook.workspace.Workspace
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json

class WorkspaceView : SurfaceView, SurfaceHolder.Callback2 {
    private var wgpuObj = Long.MAX_VALUE

    private var workspace = Workspace()

    private val frameOutputJsonParser = Json {
        ignoreUnknownKeys = true
    }

    private var redrawTask: Runnable = Runnable {
        invalidate()
    }

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
        if (wgpuObj == Long.MAX_VALUE) {
            return true
        }

        if (event != null) {
            requestFocus()

            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    workspace.touchesBegin(
                        wgpuObj,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y),
                        event.pressure
                    )
                }

                MotionEvent.ACTION_MOVE -> {
                    workspace.touchesMoved(
                        wgpuObj,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y),
                        event.pressure
                    )
                }

                MotionEvent.ACTION_UP -> {
                    workspace.touchesEnded(
                        wgpuObj,
                        event.getPointerId(0),
                        adjustTouchPoint(event.x),
                        adjustTouchPoint(event.y),
                        event.pressure
                    )
                }
            }

            invalidate()
        }

        return true
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        workspace.resizeEditor(wgpuObj, holder.surface, context.resources.displayMetrics.scaledDensity)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        holder.let { h ->
            wgpuObj = workspace.createWgpuCanvas(h.surface, CoreModel.getPtr(), textSaver!!.currentContent, context.resources.displayMetrics.scaledDensity, (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES)
            setWillNotDraw(false)
        }

        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        if (wgpuObj != Long.MAX_VALUE) {
            workspace.dropWgpuCanvas(wgpuObj)
            wgpuObj = Long.MAX_VALUE
        }
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        invalidate()
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)

        if (wgpuObj == Long.MAX_VALUE) {
            return
        }

        val responseJson = workspace.enterFrame(wgpuObj)
        val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)

        if (response.redrawIn < 100u) {
            invalidate()
        } else {
            handler.postDelayed(redrawTask, response.redrawIn.toLong())
        }
    }
}
