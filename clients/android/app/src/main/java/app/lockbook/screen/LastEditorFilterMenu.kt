package app.lockbook.screen

import android.graphics.drawable.GradientDrawable
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.PopupWindow
import android.widget.ScrollView
import android.widget.TextView
import androidx.core.view.setPadding
import app.lockbook.R
import app.lockbook.model.LastEditorFilterViewModel
import app.lockbook.util.harmonizedIdentityColors
import com.google.android.material.button.MaterialButton
import com.google.android.material.checkbox.MaterialCheckBox
import com.google.android.material.color.MaterialColors

/** A multi-select popup that stays open while editors are toggled. */
internal fun showLastEditorFilterMenu(
    anchor: View,
    model: LastEditorFilterViewModel,
): PopupWindow {
    val context = anchor.context

    fun dp(value: Int) = (value * context.resources.displayMetrics.density).toInt()
    val surface = MaterialColors.getColor(anchor, com.google.android.material.R.attr.colorSurfaceContainerHigh)
    val foreground = MaterialColors.getColor(anchor, com.google.android.material.R.attr.colorOnSurface)
    val content =
        LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(12))
        }
    content.addView(
        TextView(context).apply {
            setText(R.string.last_editor_filter_heading)
            setTextAppearance(com.google.android.material.R.style.TextAppearance_Material3_TitleSmall)
            setTextColor(foreground)
            setPadding(dp(4), dp(8), dp(4), dp(4))
            androidx.core.view.ViewCompat
                .setAccessibilityHeading(this, true)
        },
    )
    val actions = LinearLayout(context)
    content.addView(actions)
    val rows = LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
    val scroll = ScrollView(context).apply { addView(rows) }
    content.addView(scroll)

    fun renderRows() {
        rows.removeAllViews()
        val state = model.state.value!!
        state.usernames.forEachIndexed { index, username ->
            val colors = harmonizedIdentityColors(context, username)
            val row =
                LinearLayout(context).apply {
                    gravity = Gravity.CENTER_VERTICAL
                    minimumHeight = dp(48)
                }
            val avatar =
                TextView(context).apply {
                    text = username.take(1).uppercase()
                    gravity = Gravity.CENTER
                    setTextColor(colors.onAccentContainer)
                    background =
                        GradientDrawable().apply {
                            shape = GradientDrawable.OVAL
                            setColor(colors.accentContainer)
                        }
                    importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
                }
            row.addView(avatar, LinearLayout.LayoutParams(dp(28), dp(28)).apply { marginEnd = dp(12) })
            if (index == 0) {
                row.addView(
                    ImageView(context).apply {
                        setImageResource(R.drawable.ic_baseline_face_24)
                        imageTintList =
                            android.content.res.ColorStateList
                                .valueOf(colors.accent)
                        importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO
                    },
                    LinearLayout.LayoutParams(dp(20), dp(20)).apply { marginEnd = dp(4) },
                )
            }
            val checkbox =
                MaterialCheckBox(context).apply {
                    text = if (index == 0) context.getString(R.string.last_editor_filter_yourself, username) else username
                    setTextColor(foreground)
                    maxLines = 1
                    ellipsize = TextUtils.TruncateAt.END
                    isChecked = state.selected == null || username in state.selected
                    setOnCheckedChangeListener { _, _ -> model.toggle(username) }
                }
            row.addView(checkbox, LinearLayout.LayoutParams(0, dp(48), 1f))
            row.setOnClickListener { checkbox.performClick() }
            rows.addView(row)
        }
        scroll.layoutParams =
            LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(48) * state.usernames.size.coerceAtMost(6),
            )
    }

    fun action(
        label: Int,
        onClick: () -> Unit,
    ) {
        actions.addView(
            MaterialButton(context, null, androidx.appcompat.R.attr.borderlessButtonStyle).apply {
                setText(label)
                setOnClickListener {
                    onClick()
                    renderRows()
                }
            },
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f),
        )
    }
    action(R.string.last_editor_filter_select_all, model::selectAll)
    action(R.string.last_editor_filter_deselect_all, model::deselectAll)
    renderRows()
    return PopupWindow(
        content,
        minOf(dp(340), context.resources.displayMetrics.widthPixels - dp(32)),
        ViewGroup.LayoutParams.WRAP_CONTENT,
        true,
    ).apply {
        setBackgroundDrawable(
            GradientDrawable().apply {
                setColor(surface)
                cornerRadius = dp(16).toFloat()
            },
        )
        elevation = dp(8).toFloat()
        isOutsideTouchable = true
        showAsDropDown(anchor, 0, 0, Gravity.END)
    }
}
