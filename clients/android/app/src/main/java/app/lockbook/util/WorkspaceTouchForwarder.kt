package app.lockbook.util

import android.view.MotionEvent
import android.view.ViewConfiguration
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.input.motionprediction.MotionEventPredictor
import app.lockbook.util.WorkspaceView.Companion.wgpuObj
import app.lockbook.workspace.Workspace
import kotlin.math.abs

class WorkspaceTouchForwarder(
    private val workspaceView: WorkspaceView,
    private val motionEventPredictor: MotionEventPredictor,
) {
    private val touchSlop = ViewConfiguration.get(workspaceView.context).scaledTouchSlop
    private val fallbackBackGestureInset = ViewConfiguration.get(workspaceView.context).scaledEdgeSlop

    private var pendingBackGestureEvents: MutableList<MotionEvent>? = null
    private var suppressTouchStreamForBackGesture = false
    private var backGestureCandidatePointerId = MotionEvent.INVALID_POINTER_ID
    private var backGestureStartedOnLeftEdge = false
    private var backGestureStartX = 0f
    private var backGestureStartY = 0f

    fun cancelTouches(event: MotionEvent) {
        if (!workspaceView.canForwardTouches()) {
            return
        }

        for (i in 0 until event.pointerCount) {
            Workspace.touchesCancelled(
                wgpuObj,
                event.getPointerId(i),
                event.getX(i),
                event.getY(i),
                event.getPressure(i),
            )
        }
    }

    fun cancelBackGestureTouches() {
        val hadPendingBackGestureEvents = pendingBackGestureEvents != null
        recyclePendingBackGestureEvents()
        suppressTouchStreamForBackGesture = hadPendingBackGestureEvents
    }

    fun forward(
        event: MotionEvent,
        touchOffsetY: Float,
    ) {
        if (!workspaceView.canForwardTouches()) {
            return
        }

        val action = event.actionMasked

        if (suppressTouchStreamForBackGesture) {
            if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                suppressTouchStreamForBackGesture = false
                backGestureCandidatePointerId = MotionEvent.INVALID_POINTER_ID
            }
            return
        }

        pendingBackGestureEvents?.let { pendingEvents ->
            when (action) {
                MotionEvent.ACTION_MOVE -> {
                    pendingEvents.add(MotionEvent.obtain(event))
                    if (!isStillBackGestureCandidate(event)) {
                        flushPendingBackGestureEvents(touchOffsetY)
                    }
                    return
                }

                MotionEvent.ACTION_UP -> {
                    pendingEvents.add(MotionEvent.obtain(event))
                    flushPendingBackGestureEvents(touchOffsetY)
                    return
                }

                MotionEvent.ACTION_CANCEL -> {
                    recyclePendingBackGestureEvents()
                    backGestureCandidatePointerId = MotionEvent.INVALID_POINTER_ID
                    return
                }

                MotionEvent.ACTION_POINTER_DOWN,
                MotionEvent.ACTION_POINTER_UP,
                -> {
                    flushPendingBackGestureEvents(touchOffsetY)
                }
            }
        }

        if (action == MotionEvent.ACTION_DOWN && isBackGestureCandidate(event)) {
            pendingBackGestureEvents = mutableListOf(MotionEvent.obtain(event))
            backGestureCandidatePointerId = event.getPointerId(event.actionIndex)
            backGestureStartX = event.x
            backGestureStartY = event.y
            backGestureStartedOnLeftEdge = event.x <= getBackGestureInsets().first
            return
        }

        forwardImmediately(event, touchOffsetY)
    }

    private fun forwardImmediately(
        event: MotionEvent,
        touchOffsetY: Float,
    ) {
        val action = event.actionMasked
        val actionIndex = event.actionIndex
        val pressure = getEventPressure(event, actionIndex)

        when (action) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> {
                workspaceView.contextMenu?.finish()
                Workspace.touchesBegin(
                    wgpuObj,
                    event.getPointerId(actionIndex),
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure,
                )
            }

            MotionEvent.ACTION_MOVE -> {
                for (i in 0 until event.pointerCount) {
                    Workspace.touchesMoved(
                        wgpuObj,
                        event.getPointerId(i),
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
                Workspace.touchesEnded(
                    wgpuObj,
                    event.getPointerId(actionIndex),
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure,
                )
            }

            MotionEvent.ACTION_CANCEL -> {
                Workspace.touchesCancelled(
                    wgpuObj,
                    event.getPointerId(actionIndex),
                    event.getX(actionIndex),
                    event.getY(actionIndex) + touchOffsetY,
                    pressure,
                )
            }
        }

        workspaceView.invalidate()
    }

    private fun flushPendingBackGestureEvents(touchOffsetY: Float) {
        val pendingEvents = pendingBackGestureEvents ?: return
        pendingBackGestureEvents = null
        backGestureCandidatePointerId = MotionEvent.INVALID_POINTER_ID

        pendingEvents.forEach { pendingEvent ->
            forwardImmediately(pendingEvent, touchOffsetY)
            pendingEvent.recycle()
        }
    }

    private fun recyclePendingBackGestureEvents() {
        pendingBackGestureEvents?.forEach { it.recycle() }
        pendingBackGestureEvents = null
        backGestureCandidatePointerId = MotionEvent.INVALID_POINTER_ID
    }

    private fun isBackGestureCandidate(event: MotionEvent): Boolean {
        if (event.getToolType(event.actionIndex) == MotionEvent.TOOL_TYPE_STYLUS) {
            return false
        }
        if (workspaceView.width <= 0) {
            return false
        }

        val (leftInset, rightInset) = getBackGestureInsets()
        return event.x <= leftInset || event.x >= workspaceView.width - rightInset
    }

    private fun isStillBackGestureCandidate(event: MotionEvent): Boolean {
        val pointerIndex = event.findPointerIndex(backGestureCandidatePointerId)
        if (pointerIndex == -1) {
            return false
        }

        val dx = event.getX(pointerIndex) - backGestureStartX
        val dy = event.getY(pointerIndex) - backGestureStartY
        val absDx = abs(dx)
        val absDy = abs(dy)

        if (absDy > touchSlop && absDy > absDx) {
            return false
        }

        return absDx <= touchSlop ||
            if (backGestureStartedOnLeftEdge) {
                dx > 0
            } else {
                dx < 0
            }
    }

    private fun getBackGestureInsets(): Pair<Int, Int> {
        val systemGestureInsets =
            ViewCompat
                .getRootWindowInsets(workspaceView)
                ?.getInsets(WindowInsetsCompat.Type.systemGestures())

        val left = systemGestureInsets?.left?.takeIf { it > 0 } ?: fallbackBackGestureInset
        val right = systemGestureInsets?.right?.takeIf { it > 0 } ?: fallbackBackGestureInset
        return left.coerceAtLeast(1) to right.coerceAtLeast(1)
    }

    private fun getEventPressure(
        event: MotionEvent,
        actionIndex: Int,
    ): Float {
        val touchType = event.getToolType(actionIndex)

        return if (touchType == MotionEvent.TOOL_TYPE_STYLUS) {
            event.pressure * 10f // hack: on the z-fold the range is 0-0.1, uplift this to 0-1
        } else {
            Float.NaN
        }
    }
}
