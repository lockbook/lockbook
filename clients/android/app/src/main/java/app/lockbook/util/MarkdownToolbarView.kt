package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.DialogInterface
import android.content.res.ColorStateList
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.ViewGroup
import android.widget.FrameLayout
import androidx.appcompat.content.res.AppCompatResources
import androidx.core.content.edit
import androidx.core.view.isVisible
import androidx.preference.PreferenceManager
import androidx.recyclerview.widget.ItemTouchHelper
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.DialogMarkdownToolbarCustomizationBinding
import app.lockbook.databinding.ItemMarkdownToolbarActionBinding
import app.lockbook.databinding.ItemMarkdownToolbarCategoryBinding
import app.lockbook.databinding.MarkdownToolbarButtonGroupBinding
import app.lockbook.databinding.MarkdownToolbarIconButtonBinding
import app.lockbook.databinding.ViewMarkdownToolbarBinding
import app.lockbook.workspace.Workspace
import com.google.android.material.button.MaterialButton
import com.google.android.material.button.MaterialButtonGroup
import com.google.android.material.color.MaterialColors
import com.google.android.material.dialog.MaterialAlertDialogBuilder

/** Android's native control surface for the Rust markdown editor. Command IDs match toolbar.rs. */
class MarkdownToolbarView(
    context: Context,
    private val editor: WorkspaceView,
) : FrameLayout(context) {
    private enum class ToolbarCategory(val label: String) {
        History("History"),
        TextStyle("Text style"),
        Lists("Lists"),
        Attachments("Attachments"),
        Indentation("Indentation"),
    }

    private enum class ToolbarAction(
        val commandId: Int,
        val label: String,
        val icon: Int,
        val category: ToolbarCategory,
        val checkable: Boolean = false,
    ) {
        Undo(0, "Undo", R.drawable.ic_md_undo_24, ToolbarCategory.History),
        Redo(1, "Redo", R.drawable.ic_md_redo_24, ToolbarCategory.History),
        Heading(2, "Heading", R.drawable.ic_md_title_24, ToolbarCategory.TextStyle, true),
        Bold(3, "Bold", R.drawable.ic_md_format_bold_24, ToolbarCategory.TextStyle, true),
        Italic(4, "Italic", R.drawable.ic_md_format_italic_24, ToolbarCategory.TextStyle, true),
        Code(5, "Code", R.drawable.ic_md_code_24, ToolbarCategory.TextStyle, true),
        Strikethrough(6, "Strikethrough", R.drawable.ic_md_format_strikethrough_24, ToolbarCategory.TextStyle, true),
        Highlight(7, "Highlight", R.drawable.ic_md_highlight_24, ToolbarCategory.TextStyle, true),
        Underline(8, "Underline", R.drawable.ic_md_format_underlined_24, ToolbarCategory.TextStyle, true),
        Spoiler(9, "Spoiler", R.drawable.ic_md_visibility_off_24, ToolbarCategory.TextStyle, true),
        Subscript(10, "Subscript", R.drawable.ic_md_subscript_24, ToolbarCategory.TextStyle, true),
        Superscript(11, "Superscript", R.drawable.ic_md_superscript_24, ToolbarCategory.TextStyle, true),
        NumberedList(12, "Numbered list", R.drawable.ic_md_format_list_numbered_24, ToolbarCategory.Lists, true),
        BulletedList(13, "Bulleted list", R.drawable.ic_md_format_list_bulleted_24, ToolbarCategory.Lists, true),
        TaskList(14, "Task list", R.drawable.ic_md_checklist_24, ToolbarCategory.Lists, true),
        Link(15, "Link", R.drawable.ic_md_link_24, ToolbarCategory.Attachments, true),
        InsertPhoto(18, "Insert photo", R.drawable.ic_outline_camera_24, ToolbarCategory.Attachments),
        Indent(16, "Indent", R.drawable.ic_md_format_indent_increase_24, ToolbarCategory.Indentation),
        Outdent(17, "Outdent", R.drawable.ic_md_format_indent_decrease_24, ToolbarCategory.Indentation),
    }

    private data class SettingsRow(
        val category: ToolbarCategory,
        val action: ToolbarAction? = null,
    )

    private val categories = ToolbarCategory.values().toList()
    private val actions = ToolbarAction.values().toList()
    private val actionsById = actions.associateBy(ToolbarAction::commandId)
    private val prefs = PreferenceManager.getDefaultSharedPreferences(context)
    private val preferenceKey = "native_markdown_toolbar_actions_v1"
    private val orderKey = "native_markdown_toolbar_order_v1"
    private val schemaVersionKey = "native_markdown_toolbar_schema_version"
    private val inflater = LayoutInflater.from(context)
    private val binding = ViewMarkdownToolbarBinding.inflate(inflater, this, true)
    private val buttons = mutableMapOf<ToolbarAction, MaterialButton>()
    private var orderedActions = mutableListOf<ToolbarAction>()
    private var enabledActions = mutableSetOf<ToolbarAction>()

    init {
        isVisible = false
        binding.settingsButton.setOnClickListener { showSettings() }
        loadPreferences()
        rebuild()
    }

    private fun loadPreferences() {
        val savedSelection = prefs.getString(preferenceKey, null)
        enabledActions = if (savedSelection == null) {
            actions.toMutableSet()
        } else {
            decodeActions(savedSelection).toMutableSet()
        }
        if (savedSelection != null && prefs.getInt(schemaVersionKey, 1) < 2) {
            enabledActions.add(ToolbarAction.InsertPhoto)
        }

        val savedOrder = prefs.getString(orderKey, null)?.let(::decodeActions).orEmpty()
        val completeOrder = (savedOrder + actions).distinct()
        orderedActions = categories
            .flatMap { category -> completeOrder.filter { it.category == category } }
            .toMutableList()
        save()
    }

    private fun decodeActions(value: String): List<ToolbarAction> = value
        .split(',')
        .mapNotNull { it.toIntOrNull()?.let(actionsById::get) }
        .distinct()

    private fun save() {
        prefs.edit {
            putString(preferenceKey, orderedActions.filter(enabledActions::contains).joinToString(",") { it.commandId.toString() })
            putString(orderKey, orderedActions.joinToString(",") { it.commandId.toString() })
            putInt(schemaVersionKey, 2)
        }
    }

    fun refreshEditorState() {
        val ptr = WorkspaceView.wgpuObj
        if (ptr == Long.MAX_VALUE || !editor.canForwardTouches()) return
        val state = Workspace.markdownToolbarState(ptr)
        for ((action, control) in buttons) {
            val active = state and (1L shl action.commandId) != 0L
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
            if (action == ToolbarAction.Heading) {
                val level = (state ushr 32).toInt()
                control.contentDescription = if (level in 1..6) "Heading level $level" else action.label
            }
        }
    }

    private fun connectedGroup() = MarkdownToolbarButtonGroupBinding
        .inflate(inflater, binding.actionGroups, false)
        .root

    private fun iconButton(
        parent: MaterialButtonGroup,
        action: ToolbarAction,
    ) = MarkdownToolbarIconButtonBinding.inflate(inflater, parent, false).root.apply {
        contentDescription = action.label
        setIconResource(action.icon)
        isCheckable = action.checkable
        setOnClickListener {
            val ptr = WorkspaceView.wgpuObj
            if (editor.canForwardTouches()) {
                Workspace.markdownToolbarAction(ptr, action.commandId)
                editor.invalidate()
                editor.wrapperView?.requestFocus()
            }
        }
    }

    private fun rebuild() {
        binding.actionGroups.removeAllViews()
        buttons.clear()
        for (category in categories) {
            val visibleActions = orderedActions.filter { it in enabledActions && it.category == category }
            if (visibleActions.isEmpty()) continue
            val group = connectedGroup()
            for (action in visibleActions) {
                val control = iconButton(group, action)
                buttons[action] = control
                group.addView(control)
            }
            binding.actionGroups.addView(group)
        }
        refreshEditorState()
    }

    private fun showSettings() {
        val rows = mutableListOf<SettingsRow>()
        fun populateRows() {
            rows.clear()
            for (category in categories) {
                rows.add(SettingsRow(category))
                orderedActions.filter { it.category == category }.forEach { rows.add(SettingsRow(category, it)) }
            }
        }
        populateRows()

        val dialogBinding = DialogMarkdownToolbarCustomizationBinding.inflate(inflater)
        val list = dialogBinding.actionList
        list.layoutManager = LinearLayoutManager(context)
        lateinit var touchHelper: ItemTouchHelper

        class CategoryViewHolder(
            val binding: ItemMarkdownToolbarCategoryBinding,
        ) : RecyclerView.ViewHolder(binding.root)

        class ActionViewHolder(
            val binding: ItemMarkdownToolbarActionBinding,
        ) : RecyclerView.ViewHolder(binding.root)

        val adapter = object : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
            override fun getItemCount() = rows.size
            override fun getItemViewType(position: Int) = if (rows[position].action == null) 0 else 1

            override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RecyclerView.ViewHolder =
                if (viewType == 0) {
                    CategoryViewHolder(ItemMarkdownToolbarCategoryBinding.inflate(inflater, parent, false))
                } else {
                    ActionViewHolder(ItemMarkdownToolbarActionBinding.inflate(inflater, parent, false))
                }

            @SuppressLint("ClickableViewAccessibility")
            override fun onBindViewHolder(holder: RecyclerView.ViewHolder, position: Int) {
                val row = rows[position]
                val action = row.action
                if (action == null) {
                    val rowBinding = (holder as CategoryViewHolder).binding
                    rowBinding.categoryDivider.isVisible = position != 0
                    rowBinding.categoryLabel.text = row.category.label
                    return
                }

                val rowBinding = (holder as ActionViewHolder).binding
                val check = rowBinding.actionCheckbox
                val handle = rowBinding.dragHandle
                check.setOnCheckedChangeListener(null)
                check.text = action.label
                val icon = AppCompatResources.getDrawable(context, action.icon)?.mutate()
                icon?.setTint(MaterialColors.getColor(check, com.google.android.material.R.attr.colorOnSurface, 0))
                check.setCompoundDrawablesRelativeWithIntrinsicBounds(icon, null, null, null)
                check.isChecked = action in enabledActions
                check.setOnCheckedChangeListener { _, checked ->
                    if (checked) enabledActions.add(action) else enabledActions.remove(action)
                    save()
                    rebuild()
                }
                handle.contentDescription = "Drag ${action.label} to reorder"
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
                if (position == RecyclerView.NO_POSITION || rows[position].action == null) return 0
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
                val destination = rows[to]
                val movingAction = moving.action ?: return false
                val destinationAction = destination.action ?: return false
                if (moving.category != destination.category) return false
                val fromIndex = orderedActions.indexOf(movingAction)
                val toIndex = orderedActions.indexOf(destinationAction)
                if (fromIndex < 0 || toIndex < 0) return false
                orderedActions.add(toIndex, orderedActions.removeAt(fromIndex))
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
            .setView(dialogBinding.root)
            .setNeutralButton("Restore defaults", null)
            .setPositiveButton("Done", null)
            .create()
        dialog.show()
        dialog.getButton(DialogInterface.BUTTON_NEUTRAL).setOnClickListener {
            orderedActions = actions.toMutableList()
            enabledActions = actions.toMutableSet()
            save()
            rebuild()
            populateRows()
            adapter.notifyDataSetChanged()
        }
    }
}
