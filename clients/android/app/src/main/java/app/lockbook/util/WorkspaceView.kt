package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.Configuration
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.os.Bundle
import android.os.Parcelable
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
        println("surface is created!")
        holder.let { h ->
            Timber.e("Creating wgpu obj")
            WGPU_OBJ = WORKSPACE.createWgpuCanvas(
                h.surface,
                CoreModel.getPtr(),
                context.resources.displayMetrics.scaledDensity,
                (context.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES,
                WS_OBJ
            )
            Timber.e("Finished creating wgpu obj")

            setWillNotDraw(false)
        }

        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        WS_OBJ = WORKSPACE.dropWgpuCanvas(WGPU_OBJ)
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        invalidate()
    }

    override fun draw(canvas: Canvas) {
        super.draw(canvas)

        if (WGPU_OBJ == Long.MAX_VALUE) {
            return
        }

        Timber.e("entering frame")
        val responseJson = WORKSPACE.enterFrame(WGPU_OBJ)
        Timber.e("finished entering frame: ${responseJson}")
        val response: IntegrationOutput = frameOutputJsonParser.decodeFromString(responseJson)

        if (response.redrawIn < BigInteger("100")) {
            invalidate()
        } else {
            handler.postDelayed(redrawTask, response.redrawIn.toLong())
        }
    }

    companion object {
        var WGPU_OBJ = Long.MAX_VALUE
        private var WS_OBJ = Long.MAX_VALUE

        val WORKSPACE = Workspace()

        private const val WGPU_OBJ_NAME = "wgpuObj"
        private const val SUPER_STATE_KEY = "SUPER_STATE_KEY"
    }
}
