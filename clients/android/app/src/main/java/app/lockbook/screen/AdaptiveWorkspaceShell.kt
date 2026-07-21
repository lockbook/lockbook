package app.lockbook.screen

import android.content.Context
import android.util.AttributeSet
import android.view.View
import android.view.ViewGroup
import kotlin.math.roundToInt

private const val SPLIT_MIN_WIDTH_DP = 700
private const val SIDEBAR_WIDTH_DP = 300

enum class WorkspaceShellMode {
    SidebarOnly,
    DetailOnly,
    Split,
}

private enum class WorkspaceShellRequest {
    Auto,
    SidebarOnly,
    DetailVisible,
    DetailOnly,
    Split,
}

class AdaptiveWorkspaceShell
    @JvmOverloads
    constructor(
        context: Context,
        attrs: AttributeSet? = null,
        defStyleAttr: Int = 0,
    ) : ViewGroup(context, attrs, defStyleAttr) {
        var mode: WorkspaceShellMode = WorkspaceShellMode.SidebarOnly
            private set

        val isSidebarOnly: Boolean
            get() = mode == WorkspaceShellMode.SidebarOnly

        val isDetailOnly: Boolean
            get() = mode == WorkspaceShellMode.DetailOnly

        val isDetailVisible: Boolean
            get() = mode != WorkspaceShellMode.SidebarOnly

        val isSplit: Boolean
            get() = mode == WorkspaceShellMode.Split

        private var request = WorkspaceShellRequest.Auto
        private var onModeChanged: ((WorkspaceShellMode) -> Unit)? = null

        private val sidebar: View
            get() = getChildAt(SIDEBAR_CHILD_INDEX)

        private val detail: View
            get() = getChildAt(DETAIL_CHILD_INDEX)

        fun setOnModeChangedListener(listener: (WorkspaceShellMode) -> Unit) {
            onModeChanged = listener
        }

        fun showSidebar() {
            request = WorkspaceShellRequest.SidebarOnly
            applyResolvedMode()
        }

        fun showDetail() {
            request = WorkspaceShellRequest.DetailVisible
            applyResolvedMode()
        }

        fun focusDetail() {
            request = WorkspaceShellRequest.DetailOnly
            applyResolvedMode()
        }

        fun showSplit() {
            request = WorkspaceShellRequest.Split
            applyResolvedMode()
        }

        override fun onMeasure(
            widthMeasureSpec: Int,
            heightMeasureSpec: Int,
        ) {
            requireContentChildren()

            val width = MeasureSpec.getSize(widthMeasureSpec)
            val height = MeasureSpec.getSize(heightMeasureSpec)
            val nextMode = resolveMode(width)
            applyMode(nextMode)

            val sidebarWidth = sidebarWidthFor(nextMode, width)
            val detailWidth = detailWidthFor(nextMode, width, sidebarWidth)

            measureExactly(sidebar, sidebarWidth, height)
            measureExactly(detail, detailWidth, height)

            forEachOverlayChild { child ->
                measureExactly(child, width, height)
            }

            setMeasuredDimension(width, height)
        }

        override fun onLayout(
            changed: Boolean,
            left: Int,
            top: Int,
            right: Int,
            bottom: Int,
        ) {
            val width = right - left
            val height = bottom - top
            val sidebarWidth =
                if (mode == WorkspaceShellMode.Split) {
                    SIDEBAR_WIDTH_DP.toPx().coerceAtMost(width)
                } else {
                    width
                }

            when (mode) {
                WorkspaceShellMode.SidebarOnly -> {
                    sidebar.layout(0, 0, width, height)
                    detail.layout(width, 0, width * 2, height)
                }

                WorkspaceShellMode.DetailOnly -> {
                    sidebar.layout(-width, 0, 0, height)
                    detail.layout(0, 0, width, height)
                }

                WorkspaceShellMode.Split -> {
                    sidebar.layout(0, 0, sidebarWidth, height)
                    detail.layout(sidebarWidth, 0, width, height)
                }
            }

            forEachOverlayChild { child ->
                child.layout(0, 0, width, height)
            }
        }

        override fun generateDefaultLayoutParams(): LayoutParams = LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT)

        override fun generateLayoutParams(attrs: AttributeSet?): LayoutParams = LayoutParams(context, attrs)

        override fun generateLayoutParams(params: LayoutParams?): LayoutParams = LayoutParams(params)

        override fun checkLayoutParams(params: LayoutParams?): Boolean = params is LayoutParams

        private fun applyResolvedMode() {
            applyMode(resolveMode(width))
            requestLayout()
        }

        private fun resolveMode(widthPx: Int): WorkspaceShellMode {
            val isSplitAvailable = widthPx.toDp() >= SPLIT_MIN_WIDTH_DP
            return when (request) {
                WorkspaceShellRequest.Auto -> if (isSplitAvailable) WorkspaceShellMode.Split else WorkspaceShellMode.SidebarOnly
                WorkspaceShellRequest.SidebarOnly -> WorkspaceShellMode.SidebarOnly
                WorkspaceShellRequest.DetailVisible -> if (isSplitAvailable) WorkspaceShellMode.Split else WorkspaceShellMode.DetailOnly
                WorkspaceShellRequest.DetailOnly -> WorkspaceShellMode.DetailOnly
                WorkspaceShellRequest.Split -> if (isSplitAvailable) WorkspaceShellMode.Split else WorkspaceShellMode.DetailOnly
            }
        }

        private fun applyMode(nextMode: WorkspaceShellMode) {
            val modeChanged = mode != nextMode

            mode = nextMode

            if (modeChanged) {
                onModeChanged?.invoke(nextMode)
            }
        }

        private fun sidebarWidthFor(
            mode: WorkspaceShellMode,
            totalWidth: Int,
        ): Int =
            when (mode) {
                WorkspaceShellMode.SidebarOnly -> totalWidth
                WorkspaceShellMode.DetailOnly -> totalWidth
                WorkspaceShellMode.Split -> SIDEBAR_WIDTH_DP.toPx().coerceAtMost(totalWidth)
            }

        private fun detailWidthFor(
            mode: WorkspaceShellMode,
            totalWidth: Int,
            sidebarWidth: Int,
        ): Int =
            when (mode) {
                WorkspaceShellMode.SidebarOnly -> totalWidth
                WorkspaceShellMode.DetailOnly -> totalWidth
                WorkspaceShellMode.Split -> (totalWidth - sidebarWidth).coerceAtLeast(0)
            }

        private fun measureExactly(
            child: View,
            width: Int,
            height: Int,
        ) {
            child.measure(
                MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY),
                MeasureSpec.makeMeasureSpec(height, MeasureSpec.EXACTLY),
            )
        }

        private fun forEachOverlayChild(action: (View) -> Unit) {
            for (index in OVERLAY_CHILD_START_INDEX until childCount) {
                action(getChildAt(index))
            }
        }

        private fun requireContentChildren() {
            check(childCount >= OVERLAY_CHILD_START_INDEX) {
                "AdaptiveWorkspaceShell requires sidebar and detail children as its first two children."
            }
        }

        private fun Int.toDp(): Int = (this / resources.displayMetrics.density).roundToInt()

        private fun Int.toPx(): Int = (this * resources.displayMetrics.density).roundToInt()

        private companion object {
            const val SIDEBAR_CHILD_INDEX = 0
            const val DETAIL_CHILD_INDEX = 1
            const val OVERLAY_CHILD_START_INDEX = 2
        }
    }
