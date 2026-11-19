package app.lockbook.ui

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.util.AttributeSet
import android.view.View
import androidx.core.content.ContextCompat
import androidx.core.graphics.ColorUtils
import app.lockbook.R
import com.google.android.material.color.MaterialColors
import net.lockbook.Usage

class StorageInks(
    context: Context,
) {
    val synced = MaterialColors.getColor(context, androidx.appcompat.R.attr.colorPrimary, 0)
    val added = ContextCompat.getColor(context, R.color.storage_unsynced)
    val freed = ColorUtils.setAlphaComponent(synced, 77)
    val over = MaterialColors.getColor(context, androidx.appcompat.R.attr.colorError, 0)
    val track = MaterialColors.getColor(context, com.google.android.material.R.attr.colorSurfaceContainerHighest, 0)
    val marker = MaterialColors.getColor(context, com.google.android.material.R.attr.colorOnSurface, 0)
}

class StorageBarView(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs) {
    private val inks = StorageInks(context)
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val rect = RectF()
    private val clip = Path()
    private val barHeight = resources.displayMetrics.density * 12
    private val markerWidth = resources.displayMetrics.density * 1.5f

    var usage: Usage? = null
        set(value) {
            field = value
            invalidate()
        }

    override fun onMeasure(
        widthMeasureSpec: Int,
        heightMeasureSpec: Int,
    ) {
        val height = (barHeight + resources.displayMetrics.density * 6).toInt()
        setMeasuredDimension(MeasureSpec.getSize(widthMeasureSpec), resolveSize(height, heightMeasureSpec))
    }

    override fun onDraw(canvas: Canvas) {
        val usage = usage ?: return
        val synced = usage.synced.exact.toDouble()
        val local = usage.local.exact.toDouble()
        val added = usage.added.exact.toDouble()
        val freed = usage.freed.exact.toDouble()
        val cap = usage.dataCap?.exact?.toDouble()
        val scale = maxOf(cap ?: 0.0, synced + added, 1.0)
        val width = width.toFloat()
        val top = (height - barHeight) / 2
        val bottom = top + barHeight
        val radius = barHeight / 2

        fun x(value: Double) = width * (value / scale).coerceIn(0.0, 1.0).toFloat()

        paint.color = inks.track
        rect.set(0f, top, width, bottom)
        canvas.drawRoundRect(rect, radius, radius, paint)

        val kept = local - added
        val drawn = maxOf(x(local + freed), barHeight)
        canvas.save()
        clip.reset()
        rect.set(0f, top, drawn, bottom)
        clip.addRoundRect(rect, radius, radius, Path.Direction.CW)
        canvas.clipPath(clip)
        segment(canvas, x(0.0), x(kept), top, bottom, inks.synced)
        segment(canvas, x(kept), x(local), top, bottom, inks.added)
        segment(canvas, x(local), x(local + freed), top, bottom, inks.freed)
        if (cap != null && local > cap) {
            segment(canvas, x(cap), x(local), top, bottom, inks.over)
        }
        canvas.restore()

        if (cap != null && scale > cap) {
            paint.color = inks.marker
            val at = x(cap)
            canvas.drawRect(at - markerWidth / 2, 0f, at + markerWidth / 2, height.toFloat(), paint)
        }
    }

    private fun segment(
        canvas: Canvas,
        from: Float,
        to: Float,
        top: Float,
        bottom: Float,
        color: Int,
    ) {
        if (to <= from) {
            return
        }
        paint.color = color
        canvas.drawRect(from, top, to, bottom, paint)
    }
}

class FractionBarView(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs) {
    private val inks = StorageInks(context)
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    private val rect = RectF()
    private var fraction = 0.0
    private var color = inks.synced

    fun set(
        fraction: Double,
        color: Int,
    ) {
        this.fraction = fraction.coerceIn(0.0, 1.0)
        this.color = color
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        val radius = height / 2f
        paint.color = inks.track
        rect.set(0f, 0f, width.toFloat(), height.toFloat())
        canvas.drawRoundRect(rect, radius, radius, paint)
        paint.color = color
        rect.set(0f, 0f, maxOf(width * fraction.toFloat(), height.toFloat()), height.toFloat())
        canvas.drawRoundRect(rect, radius, radius, paint)
    }
}
