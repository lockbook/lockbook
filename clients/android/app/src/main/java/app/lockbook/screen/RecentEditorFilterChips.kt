package app.lockbook.screen

import android.content.Context
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.view.LayoutInflater
import android.view.View
import androidx.core.view.isVisible
import app.lockbook.R
import app.lockbook.databinding.FragmentRecentFilesBinding
import app.lockbook.model.LastEditorFilterState
import app.lockbook.model.LastEditorFilterViewModel
import app.lockbook.util.harmonizedIdentityColors
import com.google.android.material.chip.Chip
import com.google.android.material.chip.ChipGroup

/** All editors stay in the row, with Material's checked state indicating included files. */
internal class RecentEditorFilterChips(
    private val binding: FragmentRecentFilesBinding,
    private val model: LastEditorFilterViewModel,
) {
    fun render(state: LastEditorFilterState) {
        binding.recentEditorFilters.isVisible = state.isVisible
        val group = binding.recentEditorChips
        val usernames = (0 until group.childCount).map { group.getChildAt(it).tag }
        if (usernames != state.usernames) {
            group.removeAllViews()
            state.usernames.forEach { username ->
                group.addView(
                    createChip(group, state, username).apply {
                        tag = username
                        setOnClickListener { model.toggle(username) }
                    },
                )
            }
        }
        // Update in place so toggling keeps horizontal position and accessibility focus.
        for (index in 0 until group.childCount) {
            val chip = group.getChildAt(index) as Chip
            val selected = state.selected == null || chip.tag in state.selected
            chip.isChecked = selected
            chip.isChipIconVisible = !selected
            chip.chipIconSize = (if (selected) 18f else 24f) * chip.resources.displayMetrics.density
        }
    }
}

private fun createChip(
    group: ChipGroup,
    state: LastEditorFilterState,
    username: String,
): Chip =
    (LayoutInflater.from(group.context).inflate(R.layout.last_editor_filter_chip, group, false) as Chip).apply {
        id = View.generateViewId()
        text = if (username == state.usernames.firstOrNull()) context.getString(R.string.recent_editor_filter_yourself) else username
        if (username == state.usernames.firstOrNull()) {
            contentDescription = context.getString(R.string.last_editor_filter_yourself, username)
        }
        chipIcon = EditorAvatar(context, username)
        chipIconTint = null
    }

private class EditorAvatar(
    context: Context,
    username: String,
) : Drawable() {
    private val colors = harmonizedIdentityColors(context, username)
    private val initial = username.take(1).uppercase()
    private var drawableAlpha = 255
    private val paint =
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textAlign = Paint.Align.CENTER
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        }

    override fun draw(canvas: Canvas) {
        val cx = bounds.exactCenterX()
        val cy = bounds.exactCenterY()
        paint.color = colors.accentContainer
        paint.alpha = drawableAlpha
        canvas.drawCircle(cx, cy, minOf(bounds.width(), bounds.height()) / 2f, paint)
        paint.color = colors.onAccentContainer
        paint.alpha = drawableAlpha
        paint.textSize = bounds.height() * 0.55f
        canvas.drawText(initial, cx, cy - (paint.ascent() + paint.descent()) / 2f, paint)
    }

    override fun setAlpha(alpha: Int) {
        drawableAlpha = alpha
        invalidateSelf()
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
        invalidateSelf()
    }

    @Deprecated("Deprecated in Android")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
