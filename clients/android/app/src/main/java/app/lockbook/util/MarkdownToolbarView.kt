package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.DialogInterface
import android.content.res.ColorStateList
import android.view.Gravity
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.HorizontalScrollView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.content.res.AppCompatResources
import androidx.core.content.edit
import androidx.core.view.isVisible
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.ItemTouchHelper
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.workspace.Workspace
import com.google.android.material.button.MaterialButton
import com.google.android.material.button.MaterialButtonGroup
import com.google.android.material.checkbox.MaterialCheckBox
import com.google.android.material.color.MaterialColors
import com.google.android.material.dialog.MaterialAlertDialogBuilder

/** Android's native control surface for the Rust markdown editor. IDs match toolbar.rs. */
class MarkdownToolbarView(
    context: Context,
    private val editor: WorkspaceView,
) : FrameLayout(context) {
    private data class Action(val id: Int, val label: String, val icon: Int)
    private data class SettingsRow(val category: String, val actionId: Int? = null)

    private val actions = listOf(
        Action(0, "Undo", R.drawable.ic_md_undo_24), Action(1, "Redo", R.drawable.ic_md_redo_24),
        Action(2, "Heading", R.drawable.ic_md_title_24), Action(3, "Bold", R.drawable.ic_md_format_bold_24),
        Action(4, "Italic", R.drawable.ic_md_format_italic_24), Action(5, "Code", R.drawable.ic_md_code_24),
        Action(6, "Strikethrough", R.drawable.ic_md_format_strikethrough_24), Action(7, "Highlight", R.drawable.ic_md_highlight_24),
        Action(8, "Underline", R.drawable.ic_md_format_underlined_24), Action(9, "Spoiler", R.drawable.ic_md_visibility_off_24),
        Action(10, "Subscript", R.drawable.ic_md_subscript_24), Action(11, "Superscript", R.drawable.ic_md_superscript_24),
        Action(12, "Numbered list", R.drawable.ic_md_format_list_numbered_24), Action(13, "Bulleted list", R.drawable.ic_md_format_list_bulleted_24),
        Action(14, "Task list", R.drawable.ic_md_checklist_24), Action(15, "Link", R.drawable.ic_md_link_24),
        Action(18, "Insert photo", R.drawable.ic_outline_camera_24),
        Action(16, "Indent", R.drawable.ic_md_format_indent_increase_24), Action(17, "Outdent", R.drawable.ic_md_format_indent_decrease_24),
    )
    private fun category(id: Int) = when (id) {
        in 0..1 -> "History"
        in 2..11 -> "Text style"
        in 12..14 -> "Lists"
        15, 18 -> "Attachments"
        else -> "Indentation"
    }
    private val categories = listOf("History", "Text style", "Lists", "Attachments", "Indentation")
    private val prefs = PreferenceManager.getDefaultSharedPreferences(context)
    private val preferenceKey = "native_markdown_toolbar_actions_v1"
    private val orderKey = "native_markdown_toolbar_order_v1"
    private val schemaVersionKey = "native_markdown_toolbar_schema_version"
    private val row = LinearLayout(context).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
    }
    private val buttons = mutableMapOf<Int, MaterialButton>()
    private var orderedIds: MutableList<Int>? = null
    private var enabledIds: MutableSet<Int>? = null
    private var lastState = Long.MIN_VALUE

    private val dp get() = resources.displayMetrics.density
    private fun pixels(value: Int) = (value * dp).toInt()
    private fun connectedGroup() = LayoutInflater.from(context)
        .inflate(R.layout.markdown_toolbar_button_group, row, false) as MaterialButtonGroup

    init {
        val surface = MaterialColors.getColor(this, com.google.android.material.R.attr.colorSurfaceContainerLowest, 0)
        setBackgroundColor(surface)
        elevation = pixels(4).toFloat()
        isVisible = false

        val scroll = HorizontalScrollView(context).apply {
            isHorizontalScrollBarEnabled = false
            addView(row, ViewGroup.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.MATCH_PARENT))
        }
        addView(scroll, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT).apply { marginEnd = pixels(48) })
        val settingsGroup = connectedGroup().apply {
            addView(
                iconButton("Customize toolbar", R.drawable.ic_baseline_more_vert_24) { showSettings() },
                MaterialButtonGroup.LayoutParams(pixels(40), pixels(40)),
            )
        }
        addView(settingsGroup, LayoutParams(pixels(40), pixels(40), Gravity.END or Gravity.CENTER_VERTICAL).apply {
            marginEnd = pixels(4)
        })
    }

    fun refreshFromWorkspace() {
        val ptr = WorkspaceView.wgpuObj
        if (ptr == Long.MAX_VALUE || !editor.canForwardTouches()) return
        if (orderedIds == null) {
            val saved = prefs.getString(preferenceKey, null)
            val initial = saved ?: Workspace.markdownToolbarLegacyIds(ptr)
            val selected = initial.split(',').mapNotNull { it.toIntOrNull() }.filter { id -> actions.any { it.id == id } }.distinct().toMutableList()
            if (saved != null && prefs.getInt(schemaVersionKey, 1) < 2) selected.add(18)
            val savedOrder = prefs.getString(orderKey, null)?.split(',')?.mapNotNull { it.toIntOrNull() }.orEmpty()
            val allIds = (savedOrder + actions.map { it.id }).distinct().filter { id -> actions.any { it.id == id } }
            orderedIds = categories.flatMap { group -> allIds.filter { category(it) == group } }.toMutableList()
            enabledIds = selected.toMutableSet()
            save()
            rebuild()
        }
        val state = Workspace.markdownToolbarState(ptr)
        if (state != lastState) {
            lastState = state
            for ((id, control) in buttons) {
                val active = state and (1L shl id) != 0L
                control.isChecked = active
                val color = if (active) {
                    MaterialColors.getColor(control, com.google.android.material.R.attr.colorPrimaryContainer, 0)
                } else {
                    MaterialColors.getColor(control, com.google.android.material.R.attr.colorSurfaceContainerHigh, 0)
                }
                control.backgroundTintList = ColorStateList.valueOf(color)
                control.iconTint = ColorStateList.valueOf(
                    if (active) MaterialColors.getColor(control, com.google.android.material.R.attr.colorOnPrimaryContainer, 0)
                    else MaterialColors.getColor(control, com.google.android.material.R.attr.colorOnSurface, 0),
                )
                if (id == 2) {
                    val level = (state ushr 32).toInt()
                    control.contentDescription = if (level in 1..6) "Heading level $level" else "Heading"
                }
            }
        }
    }

    private fun save() {
        prefs.edit {
            putString(preferenceKey, orderedIds.orEmpty().filter { enabledIds?.contains(it) == true }.joinToString(","))
            putString(orderKey, orderedIds.orEmpty().joinToString(","))
            putInt(schemaVersionKey, 2)
        }
    }

    private fun button(label: String, action: () -> Unit) = MaterialButton(context).apply {
        contentDescription = label
        minWidth = pixels(40)
        minimumWidth = pixels(40)
        minimumHeight = pixels(40)
        insetLeft = 0
        insetRight = 0
        insetTop = 0
        insetBottom = 0
        val container = MaterialColors.getColor(this, com.google.android.material.R.attr.colorSurfaceContainerHigh, 0)
        backgroundTintList = ColorStateList.valueOf(container)
        setOnClickListener { action() }
    }

    private fun iconButton(label: String, iconRes: Int, action: () -> Unit) = button(label, action).apply {
        text = ""
        setIconResource(iconRes)
        iconSize = pixels(20)
        iconPadding = 0
        iconGravity = MaterialButton.ICON_GRAVITY_TEXT_START
        gravity = Gravity.CENTER
        setPadding(0, 0, 0, 0)
        iconTint = ColorStateList.valueOf(
            MaterialColors.getColor(this, com.google.android.material.R.attr.colorOnSurface, 0),
        )
    }

    private fun rebuild() {
        row.removeAllViews()
        buttons.clear()
        for (groupName in categories) {
            val ids = orderedIds.orEmpty().filter { enabledIds?.contains(it) == true && category(it) == groupName }
            if (ids.isEmpty()) continue
            val group = connectedGroup()
            for (id in ids) {
                val item = actions.first { it.id == id }
                val control = iconButton(item.label, item.icon) {
                    val ptr = WorkspaceView.wgpuObj
                    if (editor.canForwardTouches()) {
                        Workspace.markdownToolbarAction(ptr, id)
                        editor.invalidate()
                        editor.wrapperView?.requestFocus()
                    }
                }
                control.isCheckable = id in 2..15
                buttons[id] = control
                group.addView(control, MaterialButtonGroup.LayoutParams(pixels(40), pixels(40)))
            }
            row.addView(group, LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, pixels(40),
            ).apply {
                marginStart = pixels(6)
                marginEnd = pixels(6)
            })
        }
        lastState = Long.MIN_VALUE
    }

    private fun showSettings() {
        val rows = mutableListOf<SettingsRow>()
        fun populateRows() {
            rows.clear()
            for (group in categories) {
                rows.add(SettingsRow(group))
                orderedIds.orEmpty().filter { category(it) == group }.forEach {
                    rows.add(SettingsRow(group, it))
                }
            }
        }
        populateRows()

        val list = RecyclerView(context).apply {
            layoutManager = LinearLayoutManager(context)
            setPadding(pixels(16), 0, pixels(16), 0)
            clipToPadding = false
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                (resources.displayMetrics.heightPixels * 0.6f).toInt(),
            )
        }
        lateinit var touchHelper: ItemTouchHelper
        val adapter = object : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
            override fun getItemCount() = rows.size
            override fun getItemViewType(position: Int) = if (rows[position].actionId == null) 0 else 1

            override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RecyclerView.ViewHolder {
                val view = if (viewType == 0) {
                    LinearLayout(context).apply {
                        orientation = LinearLayout.VERTICAL
                        layoutParams = RecyclerView.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            ViewGroup.LayoutParams.WRAP_CONTENT,
                        )
                        addView(View(context).apply {
                            setBackgroundColor(MaterialColors.getColor(
                                this, com.google.android.material.R.attr.colorOutlineVariant, 0,
                            ))
                        }, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, pixels(1)))
                        addView(TextView(context).apply {
                            textSize = 14f
                            setTextColor(MaterialColors.getColor(
                                this, androidx.appcompat.R.attr.colorPrimary, 0,
                            ))
                            setPadding(pixels(8), pixels(12), 0, pixels(4))
                        })
                    }
                } else {
                    LinearLayout(context).apply {
                        gravity = Gravity.CENTER_VERTICAL
                        layoutParams = RecyclerView.LayoutParams(
                            ViewGroup.LayoutParams.MATCH_PARENT,
                            pixels(48),
                        )
                        addView(MaterialCheckBox(context), LinearLayout.LayoutParams(0, pixels(48), 1f))
                        addView(iconButton("Drag to reorder", R.drawable.ic_md_drag_indicator_24) {},
                            LinearLayout.LayoutParams(pixels(48), pixels(48)))
                    }
                }
                return object : RecyclerView.ViewHolder(view) {}
            }

            @SuppressLint("ClickableViewAccessibility")
            override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
                val row = rows[position]
                val id = row.actionId
                if (id == null) {
                    val header = holder.itemView as LinearLayout
                    header.getChildAt(0).visibility = if (position == 0) View.GONE else View.VISIBLE
                    (header.getChildAt(1) as TextView).text = row.category
                    return
                }
                val item = actions.first { it.id == id }
                val line = holder.itemView as LinearLayout
                val check = line.getChildAt(0) as MaterialCheckBox
                val handle = line.getChildAt(1) as MaterialButton
                check.setOnCheckedChangeListener(null)
                check.text = item.label
                val icon = AppCompatResources.getDrawable(context, item.icon)?.mutate()
                icon?.setTint(MaterialColors.getColor(check, com.google.android.material.R.attr.colorOnSurface, 0))
                check.setCompoundDrawablesRelativeWithIntrinsicBounds(icon, null, null, null)
                check.compoundDrawablePadding = pixels(8)
                check.isChecked = enabledIds?.contains(id) == true
                check.setOnCheckedChangeListener { _, checked ->
                    if (checked) enabledIds?.add(id) else enabledIds?.remove(id)
                    save()
                    rebuild()
                }
                handle.contentDescription = "Drag ${item.label} to reorder"
                handle.setOnTouchListener { _, event ->
                    if (event.actionMasked == MotionEvent.ACTION_DOWN) touchHelper.startDrag(holder)
                    false
                }
            }
        }
        list.adapter = adapter
        touchHelper = ItemTouchHelper(object : ItemTouchHelper.SimpleCallback(
            ItemTouchHelper.UP or ItemTouchHelper.DOWN, 0,
        ) {
            override fun isLongPressDragEnabled() = false

            override fun getMovementFlags(recyclerView: RecyclerView, viewHolder: RecyclerView.ViewHolder): Int {
                val position = viewHolder.bindingAdapterPosition
                if (position == RecyclerView.NO_POSITION || rows[position].actionId == null) return 0
                return makeMovementFlags(ItemTouchHelper.UP or ItemTouchHelper.DOWN, 0)
            }

            override fun onMove(
                recyclerView: RecyclerView,
                viewHolder: RecyclerView.ViewHolder,
                target: RecyclerView.ViewHolder,
            ): Boolean {
                val from = viewHolder.bindingAdapterPosition
                val to = target.bindingAdapterPosition
                if (from == RecyclerView.NO_POSITION || to == RecyclerView.NO_POSITION) return false
                val moving = rows[from]
                if (moving.actionId == null || rows[to].actionId == null || moving.category != rows[to].category) return false
                val ids = orderedIds ?: return false
                val fromIndex = ids.indexOf(moving.actionId)
                val toIndex = ids.indexOf(rows[to].actionId)
                if (fromIndex < 0 || toIndex < 0) return false
                ids.add(toIndex, ids.removeAt(fromIndex))
                rows.add(to, rows.removeAt(from))
                adapter.notifyItemMoved(from, to)
                return true
            }

            override fun onSwiped(viewHolder: RecyclerView.ViewHolder, direction: Int) = Unit

            override fun clearView(recyclerView: RecyclerView, viewHolder: RecyclerView.ViewHolder) {
                super.clearView(recyclerView, viewHolder)
                save()
                rebuild()
            }
        })
        touchHelper.attachToRecyclerView(list)
        val dialog = MaterialAlertDialogBuilder(context, R.style.AppTheme_AlertDialog_FilledDone)
            .setTitle("Customize toolbar")
            .setView(list)
            .setNeutralButton("Restore defaults", null)
            .setPositiveButton("Done", null)
            .create()
        dialog.show()
        dialog.getButton(DialogInterface.BUTTON_NEUTRAL).setOnClickListener {
            orderedIds = actions.map { it.id }.toMutableList()
            enabledIds = orderedIds!!.toMutableSet()
            save()
            rebuild()
            populateRows()
            adapter.notifyDataSetChanged()
        }
    }
}
