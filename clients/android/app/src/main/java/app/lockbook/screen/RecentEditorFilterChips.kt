package app.lockbook.screen

import android.content.Context
import android.graphics.Canvas
import android.graphics.ColorFilter
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.view.LayoutInflater
import androidx.appcompat.app.AlertDialog
import androidx.core.view.isVisible
import androidx.lifecycle.LifecycleOwner
import app.lockbook.R
import app.lockbook.databinding.DialogRecentEditorFilterBinding
import app.lockbook.databinding.FragmentRecentFilesBinding
import app.lockbook.model.LastEditorFilterState
import app.lockbook.model.LastEditorFilterViewModel
import app.lockbook.util.harmonizedIdentityColors
import com.google.android.material.chip.Chip
import com.google.android.material.chip.ChipGroup
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.snackbar.Snackbar

/** Recents' active editor chips and a modal containing the complete editor selection. */
internal class RecentEditorFilterChips(
    private val binding: FragmentRecentFilesBinding,
    private val model: LastEditorFilterViewModel,
    private val lifecycleOwner: LifecycleOwner,
    private val snackbarAnchor: android.view.View,
) {
    private var dialog: AlertDialog? = null
    private var snackbar: Snackbar? = null
    private var renderedState: LastEditorFilterState? = null

    init {
        binding.recentEditorFilter.setOnClickListener { showDialog() }
    }

    fun render(state: LastEditorFilterState) {
        if (state == renderedState) return
        renderedState = state
        binding.recentEditorFilters.isVisible = state.isVisible
        if (!state.isVisible) dialog?.dismiss()
        val group = binding.recentEditorChips
        // Keep the first chip, including its accessibility focus and click listener.
        while (group.childCount > 1) group.removeViewAt(1)
        state.usernames.filter(state::includes).forEach { username ->
            val chip =
                createChip(group, state, username).apply {
                    isCheckable = false
                    isClickable = false
                    isCloseIconVisible = true
                    closeIconContentDescription = context.getString(R.string.recent_editor_filter_hide, username)
                    setOnCloseIconClickListener {
                        model.setEnabled(username, false)
                        snackbar?.dismiss()
                        snackbar =
                            Snackbar
                                .make(
                                    binding.root,
                                    context.getString(R.string.recent_editor_filter_hidden, username),
                                    Snackbar.LENGTH_LONG,
                                ).setAnchorView(snackbarAnchor)
                                .setAction(R.string.undo) { model.setEnabled(username, true) }
                                .also { it.show() }
                    }
                }
            group.addView(chip)
        }
    }

    private fun showDialog() {
        dialog?.dismiss()
        val context = binding.root.context
        val content = DialogRecentEditorFilterBinding.inflate(LayoutInflater.from(context))
        val modal =
            MaterialAlertDialogBuilder(context)
                .setTitle(R.string.recent_editor_filter_heading)
                .setView(content.root)
                .create()
        dialog = modal

        fun update(state: LastEditorFilterState) {
            // Update in place on toggles so keyboard and accessibility focus remain on the chip.
            val group = content.editorChips
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
            for (index in 0 until group.childCount) {
                val chip = group.getChildAt(index) as Chip
                chip.isChecked = state.selected == null || chip.tag in state.selected
            }
        }
        val observer = androidx.lifecycle.Observer<LastEditorFilterState> { update(it) }
        model.state.observe(lifecycleOwner, observer)
        content.selectAll.setOnClickListener { model.selectAll() }
        content.deselectAll.setOnClickListener { model.deselectAll() }
        modal.setOnDismissListener {
            model.state.removeObserver(observer)
            if (dialog === modal) dialog = null
        }
        modal.show()
    }

    fun dispose() {
        dialog?.dismiss()
        snackbar?.dismiss()
    }
}

private fun createChip(
    group: ChipGroup,
    state: LastEditorFilterState,
    username: String,
): Chip =
    (LayoutInflater.from(group.context).inflate(R.layout.last_editor_filter_chip, group, false) as Chip).apply {
        id = android.view.View.generateViewId()
        text = if (username == state.usernames.firstOrNull()) context.getString(R.string.recent_editor_filter_yourself) else username
        if (username == state.usernames.firstOrNull()) {
            contentDescription = context.getString(R.string.last_editor_filter_yourself, username)
        }
        chipIcon = EditorAvatar(context, username)
        chipIconTint = null
        isChecked = state.selected == null || username in state.selected
    }

private class EditorAvatar(
    context: Context,
    username: String,
) : Drawable() {
    private val colors = harmonizedIdentityColors(context, username)
    private val initial = username.take(1).uppercase()
    private val paint =
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            textAlign = Paint.Align.CENTER
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        }

    override fun draw(canvas: Canvas) {
        val cx = bounds.exactCenterX()
        val cy = bounds.exactCenterY()
        paint.color = colors.accentContainer
        canvas.drawCircle(cx, cy, minOf(bounds.width(), bounds.height()) / 2f, paint)
        paint.color = colors.onAccentContainer
        paint.textSize = bounds.height() * 0.55f
        canvas.drawText(initial, cx, cy - (paint.ascent() + paint.descent()) / 2f, paint)
    }

    override fun setAlpha(alpha: Int) {
        paint.alpha = alpha
    }

    override fun setColorFilter(colorFilter: ColorFilter?) {
        paint.colorFilter = colorFilter
    }

    @Deprecated("Deprecated in Android")
    override fun getOpacity(): Int = PixelFormat.TRANSLUCENT
}
