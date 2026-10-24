package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.view.MotionEvent
import android.view.View
import com.google.android.material.color.MaterialColors
import com.google.android.material.shape.MaterialShapeDrawable
import kotlin.math.max
import kotlin.math.min

/** A grid of table sizes: the block of cells up to the finger lights up, a caption says its size, and a lift picks it. */
class TableSizePickerView(
    context: Context,
) : View(context) {
    var onPick: ((columns: Int, rows: Int) -> Unit)? = null
    var onCancel: (() -> Unit)? = null

    private val columns = 10
    private val rows = 8
    private val density = resources.displayMetrics.density
    private val cell = 22 * density
    private val gap = 5 * density
    private val step = cell + gap
    private val padding = 12 * density
    private val captionHeight = 24 * density
    private var picked: Pair<Int, Int>? = null
    private val cellRect = RectF()

    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }
    private val edge =
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = density
        }
    private val caption =
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textSize = 13 * density
            textAlign = Paint.Align.CENTER
            color = MaterialColors.getColor(this@TableSizePickerView, com.google.android.material.R.attr.colorOnSurfaceVariant)
        }
    private val accent = MaterialColors.getColor(this, androidx.appcompat.R.attr.colorPrimary)
    private val litFill = MaterialColors.getColor(this, com.google.android.material.R.attr.colorPrimaryContainer)
    private val restFill = MaterialColors.getColor(this, com.google.android.material.R.attr.colorSurfaceContainerHigh)
    private val restEdge = MaterialColors.getColor(this, com.google.android.material.R.attr.colorOutlineVariant)

    init {
        background =
            MaterialShapeDrawable().apply {
                fillColor =
                    android.content.res.ColorStateList.valueOf(
                        MaterialColors.getColor(this@TableSizePickerView, com.google.android.material.R.attr.colorSurfaceContainer),
                    )
                setCornerSize(12 * density)
            }
    }

    override fun onMeasure(
        widthMeasureSpec: Int,
        heightMeasureSpec: Int,
    ) {
        val width = (2 * padding + columns * step - gap).toInt()
        val height = (2 * padding + rows * step - gap + captionHeight).toInt()
        setMeasuredDimension(width, height)
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val litColumns = picked?.first ?: 0
        val litRows = picked?.second ?: 0
        val rect = cellRect
        for (row in 0 until rows) {
            for (column in 0 until columns) {
                val left = padding + column * step
                val top = padding + row * step
                rect.set(left, top, left + cell, top + cell)
                val lit = column < litColumns && row < litRows
                fill.color = if (lit) litFill else restFill
                edge.color = if (lit) accent else restEdge
                canvas.drawRoundRect(rect, 3 * density, 3 * density, fill)
                canvas.drawRoundRect(rect, 3 * density, 3 * density, edge)
            }
        }
        val text = picked?.let { (c, r) -> "$c × $r" } ?: "Table size"
        val baseline = height - padding - (captionHeight - caption.textSize) / 2
        canvas.drawText(text, width / 2f, baseline, caption)
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_MOVE -> {
                val column = ((event.x - padding) / step).toInt()
                val row = ((event.y - padding) / step).toInt()
                picked = (min(max(column, 0), columns - 1) + 1) to (min(max(row, 0), rows - 1) + 1)
                invalidate()
            }

            // a lift off the grid picks nothing
            MotionEvent.ACTION_UP -> {
                val inside = event.x in padding..(width - padding) && event.y in padding..(padding + rows * step - gap)
                picked?.takeIf { inside }?.let { (c, r) -> onPick?.invoke(c, r) } ?: onCancel?.invoke()
            }

            MotionEvent.ACTION_CANCEL -> {
                picked = null
                invalidate()
            }
        }
        return true
    }
}
