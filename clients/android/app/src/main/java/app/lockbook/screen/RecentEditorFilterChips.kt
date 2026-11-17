package app.lockbook.screen

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.graphics.ColorUtils
import androidx.core.view.doOnLayout
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.RecentEditorFiltersBinding
import app.lockbook.model.LastEditorFilterState
import app.lockbook.model.LastEditorFilterViewModel
import app.lockbook.util.harmonizedIdentityColors
import com.google.android.material.chip.Chip
import com.google.android.material.chip.ChipGroup
import com.google.android.material.color.MaterialColors

/** A list header so editor filters scroll vertically with the date groups. */
internal class RecentEditorFilterAdapter(
    private val model: LastEditorFilterViewModel,
) : RecyclerView.Adapter<RecentEditorFilterAdapter.Holder>() {
    private var state = LastEditorFilterState()
    private var horizontalOffset = 0

    fun submitState(updated: LastEditorFilterState) {
        if (state == updated) return
        val wasVisible = state.isVisible
        state = updated
        when {
            !wasVisible && updated.isVisible -> notifyItemInserted(0)
            wasVisible && !updated.isVisible -> notifyItemRemoved(0)
            updated.isVisible -> notifyItemChanged(0)
        }
    }

    override fun getItemCount(): Int = if (state.isVisible) 1 else 0

    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): Holder {
        val binding = RecentEditorFiltersBinding.inflate(LayoutInflater.from(parent.context), parent, false)
        return Holder(binding, model)
    }

    override fun onBindViewHolder(
        holder: Holder,
        position: Int,
    ) {
        holder.chips.render(state)
        holder.binding.root.doOnLayout { it.scrollTo(horizontalOffset, 0) }
        holder.binding.root.setOnScrollChangeListener { _, x, _, _, _ -> horizontalOffset = x }
    }

    class Holder(
        val binding: RecentEditorFiltersBinding,
        model: LastEditorFilterViewModel,
    ) : RecyclerView.ViewHolder(binding.root) {
        val chips = RecentEditorFilterChips(binding, model)
    }
}

/** All editors stay in the row, with Material's checked state indicating included files. */
internal class RecentEditorFilterChips(
    private val binding: RecentEditorFiltersBinding,
    private val model: LastEditorFilterViewModel,
) {
    fun render(state: LastEditorFilterState) {
        val group = binding.recentEditorChips
        val usernames = (0 until group.childCount).map { group.getChildAt(it).tag }
        if (usernames != state.usernames) {
            group.removeAllViews()
            state.usernames.forEach { username ->
                group.addView(
                    createChip(group, username).apply {
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
        }
    }
}

private fun createChip(
    group: ChipGroup,
    username: String,
): Chip =
    (LayoutInflater.from(group.context).inflate(R.layout.last_editor_filter_chip, group, false) as Chip).apply {
        id = View.generateViewId()
        text = username
        chipIcon = EditorAvatar(context, username)
        chipIconTint = null
        val colors = harmonizedIdentityColors(context, username)
        val surface = MaterialColors.getColor(this, com.google.android.material.R.attr.colorSurface)
        val foreground = MaterialColors.getColor(this, com.google.android.material.R.attr.colorOnSurface)
        val secondary = MaterialColors.getColor(this, com.google.android.material.R.attr.colorOnSurfaceVariant)
        // Reuse the avatar's harmonized hue as a quiet wash over the page surface.
        val selectedFill =
            ColorUtils.blendARGB(
                surface,
                colors.accentContainer,
                if (MaterialColors.isColorLight(surface)) 0.5f else 0.35f,
            )
        val states =
            arrayOf(
                intArrayOf(-android.R.attr.state_enabled),
                intArrayOf(android.R.attr.state_checked),
                intArrayOf(),
            )
        chipBackgroundColor = ColorStateList(states, intArrayOf(surface, selectedFill, surface))
        setTextColor(
            ColorStateList(
                states,
                intArrayOf(
                    ColorUtils.setAlphaComponent(foreground, 97),
                    colors.onAccentContainer,
                    secondary,
                ),
            ),
        )
        rippleColor =
            ColorStateList(
                states,
                intArrayOf(
                    android.graphics.Color.TRANSPARENT,
                    ColorUtils.setAlphaComponent(colors.onAccentContainer, 31),
                    ColorUtils.setAlphaComponent(secondary, 31),
                ),
            )
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
