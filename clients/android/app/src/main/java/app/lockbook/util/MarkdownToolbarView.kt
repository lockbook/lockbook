package app.lockbook.util

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.view.Gravity
import android.view.LayoutInflater
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.PopupWindow
import androidx.appcompat.content.res.AppCompatResources
import androidx.appcompat.widget.PopupMenu
import androidx.core.content.edit
import androidx.core.graphics.drawable.toDrawable
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
import app.lockbook.workspace.MarkdownToolbarAction
import app.lockbook.workspace.Workspace
import com.google.android.material.button.MaterialButton
import com.google.android.material.button.MaterialButtonGroup
import com.google.android.material.color.MaterialColors
import com.google.android.material.dialog.MaterialAlertDialogBuilder

/** Android's native control surface for the Rust markdown editor. */
@SuppressLint("ViewConstructor")
class MarkdownToolbarView(
    context: Context,
    private val editor: WorkspaceView,
) : FrameLayout(context) {
    private enum class ToolbarCategory(
        val label: String,
    ) {
        History("History"),
        TextStyle("Text style"),
        Lists("Lists and quotes"),
        Attachments("Attachments"),
        Indentation("Indentation"),

        /** The table set: shown in a table, and not the user's to arrange. */
        Table("Table"),
    }

    private enum class NativeAction {
        InsertPhoto,
        Outline,
    }

    private data class ToolbarAction(
        val command: MarkdownToolbarAction?,
        val label: String,
        val icon: Int,
        val category: ToolbarCategory,
        val checkable: Boolean = true,
        val nativeAction: NativeAction? = null,
    ) {
        val preferenceKey: String
            get() = command?.name ?: requireNotNull(nativeAction).name
    }

    private data class SettingsRow(
        val category: ToolbarCategory,
        val action: ToolbarAction? = null,
    )

    private val categories = ToolbarCategory.values().filter { it != ToolbarCategory.Table }
    private val actions =
        listOf(
            ToolbarAction(MarkdownToolbarAction.Undo, "Undo", R.drawable.ic_md_undo_24, ToolbarCategory.History, false),
            ToolbarAction(MarkdownToolbarAction.Redo, "Redo", R.drawable.ic_md_redo_24, ToolbarCategory.History, false),
            ToolbarAction(
                null,
                context.getString(R.string.outline),
                R.drawable.ic_md_toc_24,
                ToolbarCategory.History,
                false,
                NativeAction.Outline,
            ),
            ToolbarAction(MarkdownToolbarAction.Heading, "Heading", R.drawable.ic_md_title_24, ToolbarCategory.TextStyle),
            ToolbarAction(MarkdownToolbarAction.Bold, "Bold", R.drawable.ic_md_format_bold_24, ToolbarCategory.TextStyle),
            ToolbarAction(MarkdownToolbarAction.Italic, "Italic", R.drawable.ic_md_format_italic_24, ToolbarCategory.TextStyle),
            ToolbarAction(MarkdownToolbarAction.Code, "Code", R.drawable.ic_md_code_24, ToolbarCategory.TextStyle),
            ToolbarAction(
                MarkdownToolbarAction.Strikethrough,
                "Strikethrough",
                R.drawable.ic_md_format_strikethrough_24,
                ToolbarCategory.TextStyle,
            ),
            ToolbarAction(MarkdownToolbarAction.Highlight, "Highlight", R.drawable.ic_md_highlight_24, ToolbarCategory.TextStyle),
            ToolbarAction(MarkdownToolbarAction.Underline, "Underline", R.drawable.ic_md_format_underlined_24, ToolbarCategory.TextStyle),
            ToolbarAction(MarkdownToolbarAction.Spoiler, "Spoiler", R.drawable.ic_md_visibility_off_24, ToolbarCategory.TextStyle),
            ToolbarAction(MarkdownToolbarAction.Subscript, "Subscript", R.drawable.ic_md_subscript_24, ToolbarCategory.TextStyle),
            ToolbarAction(MarkdownToolbarAction.Superscript, "Superscript", R.drawable.ic_md_superscript_24, ToolbarCategory.TextStyle),
            ToolbarAction(
                MarkdownToolbarAction.NumberedList,
                "Numbered list",
                R.drawable.ic_md_format_list_numbered_24,
                ToolbarCategory.Lists,
            ),
            ToolbarAction(
                MarkdownToolbarAction.BulletedList,
                "Bulleted list",
                R.drawable.ic_md_format_list_bulleted_24,
                ToolbarCategory.Lists,
            ),
            ToolbarAction(MarkdownToolbarAction.TaskList, "Task list", R.drawable.ic_md_checklist_24, ToolbarCategory.Lists),
            ToolbarAction(MarkdownToolbarAction.Quote, "Quote", R.drawable.ic_md_format_quote_24, ToolbarCategory.Lists),
            ToolbarAction(MarkdownToolbarAction.Table, "Table", R.drawable.ic_md_table_24, ToolbarCategory.Lists, false),
            ToolbarAction(MarkdownToolbarAction.Link, "Link", R.drawable.ic_md_link_24, ToolbarCategory.Attachments),
            ToolbarAction(MarkdownToolbarAction.Image, "Image", R.drawable.ic_outline_image_24, ToolbarCategory.Attachments),
            ToolbarAction(
                null,
                "Insert photo",
                R.drawable.ic_outline_camera_24,
                ToolbarCategory.Attachments,
                false,
                NativeAction.InsertPhoto,
            ),
            ToolbarAction(
                MarkdownToolbarAction.Indent,
                "Indent",
                R.drawable.ic_md_format_indent_increase_24,
                ToolbarCategory.Indentation,
                false,
            ),
            ToolbarAction(
                MarkdownToolbarAction.Outdent,
                "Outdent",
                R.drawable.ic_md_format_indent_decrease_24,
                ToolbarCategory.Indentation,
                false,
            ),
        )

    /** What stands in for the block styles while the selection is in a table. */
    private val tableGroups =
        listOf(
            // a touch keyboard has no Tab
            listOf(
                tableAction(MarkdownToolbarAction.PreviousCell, "Previous cell", R.drawable.ic_md_keyboard_tab_back_24),
                tableAction(MarkdownToolbarAction.NextCell, "Next cell", R.drawable.ic_md_keyboard_tab_24),
                tableAction(MarkdownToolbarAction.NextRow, "Next row", R.drawable.ic_md_keyboard_return_24),
            ),
            listOf(
                tableAction(MarkdownToolbarAction.RowAbove, "Insert row above", R.drawable.ic_md_row_above_24),
                tableAction(MarkdownToolbarAction.RowBelow, "Insert row below", R.drawable.ic_md_row_below_24),
                tableAction(MarkdownToolbarAction.ColumnLeft, "Insert column left", R.drawable.ic_md_column_left_24),
                tableAction(MarkdownToolbarAction.ColumnRight, "Insert column right", R.drawable.ic_md_column_right_24),
            ),
            listOf(
                tableAction(MarkdownToolbarAction.DeleteRow, "Delete row", R.drawable.ic_md_row_remove_24),
                tableAction(MarkdownToolbarAction.DeleteColumn, "Delete column", R.drawable.ic_md_column_remove_24),
            ),
            listOf(
                tableAction(MarkdownToolbarAction.AlignLeft, "Align left", R.drawable.ic_md_format_align_left_24, true),
                tableAction(MarkdownToolbarAction.AlignCenter, "Align center", R.drawable.ic_md_format_align_center_24, true),
                tableAction(MarkdownToolbarAction.AlignRight, "Align right", R.drawable.ic_md_format_align_right_24, true),
            ),
        )
    private val defaultActions =
        actions.filter { action ->
            action.category != ToolbarCategory.TextStyle ||
                action.command in
                setOf(
                    MarkdownToolbarAction.Heading,
                    MarkdownToolbarAction.Bold,
                    MarkdownToolbarAction.Italic,
                )
        }
    private val actionsByKey = actions.associateBy(ToolbarAction::preferenceKey)
    private val prefs = PreferenceManager.getDefaultSharedPreferences(context)
    private val preferenceKey = "native_markdown_toolbar_actions_v3"
    private val orderKey = "native_markdown_toolbar_order_v2"
    private val inflater = LayoutInflater.from(context)
    private val binding = ViewMarkdownToolbarBinding.inflate(inflater, this, true)
    private val buttons = mutableMapOf<ToolbarAction, MaterialButton>()
    private var orderedActions = mutableListOf<ToolbarAction>()
    private var enabledActions = mutableSetOf<ToolbarAction>()
    private var inTable = false

    init {
        isVisible = false
        binding.settingsButton.setOnClickListener { showSettings() }
        loadPreferences()
        rebuild()
    }

    private fun tableAction(
        command: MarkdownToolbarAction,
        label: String,
        icon: Int,
        checkable: Boolean = false,
    ) = ToolbarAction(command, label, icon, ToolbarCategory.Table, checkable)

    private fun loadPreferences() {
        val savedSelection = prefs.getString(preferenceKey, null)
        enabledActions =
            if (savedSelection == null) {
                defaultActions.toMutableSet()
            } else {
                decodeActions(savedSelection).toMutableSet()
            }
        val savedOrder = prefs.getString(orderKey, null)?.let(::decodeActions)
        // an action newer than the saved order starts as the defaults have it
        if (savedOrder != null) enabledActions += defaultActions.filter { it !in savedOrder }
        val completeOrder = (savedOrder.orEmpty() + actions).distinct()
        orderedActions =
            categories
                .flatMap { category -> completeOrder.filter { it.category == category } }
                .toMutableList()
        save()
    }

    private fun decodeActions(value: String): List<ToolbarAction> =
        value
            .split(',')
            .mapNotNull(actionsByKey::get)
            .distinct()

    private fun save() {
        prefs.edit {
            putString(
                preferenceKey,
                orderedActions.filter(enabledActions::contains).joinToString(",", transform = ToolbarAction::preferenceKey),
            )
            putString(orderKey, orderedActions.joinToString(",", transform = ToolbarAction::preferenceKey))
        }
    }

    fun refreshEditorState() {
        val ptr = WorkspaceView.wgpuObj
        if (ptr == Long.MAX_VALUE || !editor.canForwardTouches()) return
        val state = Workspace.markdownToolbarState(ptr)
        if (state.inTable != inTable) {
            // rebuilding refreshes, by when the two agree
            inTable = state.inTable
            rebuild()
            binding.actionScroll.scrollTo(0, 0)
            return
        }
        for ((action, control) in buttons) {
            val active = action.command in state.active
            control.isEnabled =
                if (action.nativeAction == NativeAction.Outline) Workspace.outlineAvailable(ptr) else action.command !in state.disabled
            control.alpha = if (control.isEnabled) 1f else 0.38f
            control.isChecked = active
            val color =
                if (active) {
                    MaterialColors.getColor(control, com.google.android.material.R.attr.colorPrimaryContainer, 0)
                } else {
                    MaterialColors.getColor(control, com.google.android.material.R.attr.colorSurfaceContainerHigh, 0)
                }
            control.backgroundTintList = ColorStateList.valueOf(color)
            control.iconTint =
                ColorStateList.valueOf(
                    if (active) {
                        MaterialColors.getColor(control, com.google.android.material.R.attr.colorOnPrimaryContainer, 0)
                    } else {
                        MaterialColors.getColor(control, com.google.android.material.R.attr.colorOnSurface, 0)
                    },
                )
            if (action.command == MarkdownToolbarAction.Heading) {
                control.contentDescription = state.headingLevel?.let { "Heading level $it" } ?: action.label
            }
            if (action.command == MarkdownToolbarAction.DeleteRow) {
                control.contentDescription = if (state.onHeader) "Delete table" else action.label
            }
        }
    }

    private fun connectedGroup() =
        MarkdownToolbarButtonGroupBinding
            .inflate(inflater, binding.actionGroups, false)
            .root

    private fun iconButton(
        parent: MaterialButtonGroup,
        action: ToolbarAction,
    ) = MarkdownToolbarIconButtonBinding.inflate(inflater, parent, false).root.apply {
        contentDescription = action.label
        setIconResource(action.icon)
        isCheckable = action.checkable
        setOnClickListener { anchor ->
            when (action.nativeAction) {
                NativeAction.InsertPhoto -> {
                    showPhotoMenu(anchor)
                    return@setOnClickListener
                }

                NativeAction.Outline -> {
                    editor.toggleOutline()
                    return@setOnClickListener
                }

                null -> {
                    Unit
                }
            }
            val command = requireNotNull(action.command)
            if (action.command == MarkdownToolbarAction.Table) {
                showTablePicker(anchor)
                return@setOnClickListener
            }
            val ptr = WorkspaceView.wgpuObj
            if (editor.canForwardTouches()) {
                Workspace.markdownToolbarAction(ptr, command)
                editor.invalidate()
                editor.wrapperView?.requestFocus()
            }
        }
    }

    private fun showTablePicker(anchor: View) {
        val picker = TableSizePickerView(context)
        // not focusable: focus would take the keyboard, and the toolbar with it
        val popup = PopupWindow(picker, ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, false)
        popup.isOutsideTouchable = true
        popup.setBackgroundDrawable(Color.TRANSPARENT.toDrawable())
        popup.inputMethodMode = PopupWindow.INPUT_METHOD_NOT_NEEDED
        popup.elevation = resources.displayMetrics.density * 8
        picker.onCancel = { popup.dismiss() }
        picker.onPick = { columns, rows ->
            popup.dismiss()
            val ptr = WorkspaceView.wgpuObj
            if (editor.canForwardTouches()) {
                Workspace.insertTable(ptr, columns, rows)
                editor.invalidate()
                editor.wrapperView?.requestFocus()
            }
        }
        // over the button: under it is the keyboard
        picker.measure(View.MeasureSpec.UNSPECIFIED, View.MeasureSpec.UNSPECIFIED)
        val at = IntArray(2).also(anchor::getLocationInWindow)
        val gap = (8 * resources.displayMetrics.density).toInt()
        popup.showAtLocation(anchor, Gravity.NO_GRAVITY, at[0], at[1] - picker.measuredHeight - gap)
    }

    private fun showPhotoMenu(anchor: View) {
        PopupMenu(context, anchor).apply {
            menuInflater.inflate(R.menu.markdown_photo_actions, menu)
            setForceShowIcon(true)
            setOnMenuItemClickListener { item ->
                when (item.itemId) {
                    R.id.take_photo -> editor.model._takePhotoRequested.value = Unit
                    R.id.choose_photos -> editor.model._choosePhotosRequested.value = Unit
                    else -> return@setOnMenuItemClickListener false
                }
                true
            }
            show()
        }
    }

    private fun rebuild() {
        binding.actionGroups.removeAllViews()
        buttons.clear()
        for (category in categories) {
            // a table has no place for the block styles: the table set stands in for them
            val blockStyles = category == ToolbarCategory.Lists || category == ToolbarCategory.Indentation
            if (!(inTable && blockStyles)) {
                addGroup(
                    orderedActions.filter {
                        it in enabledActions &&
                            it.category == category &&
                            !(inTable && it.command == MarkdownToolbarAction.Heading)
                    },
                )
            }
            if (inTable && category == ToolbarCategory.History) tableGroups.forEach(::addGroup)
        }
        refreshEditorState()
    }

    private fun addGroup(visibleActions: List<ToolbarAction>) {
        if (visibleActions.isEmpty()) return
        val group = connectedGroup()
        for (action in visibleActions) {
            val control = iconButton(group, action)
            buttons[action] = control
            group.addView(control)
        }
        binding.actionGroups.addView(group)
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

        val adapter =
            object : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
                override fun getItemCount() = rows.size

                override fun getItemViewType(position: Int) = if (rows[position].action == null) 0 else 1

                override fun onCreateViewHolder(
                    parent: ViewGroup,
                    viewType: Int,
                ): RecyclerView.ViewHolder =
                    if (viewType == 0) {
                        CategoryViewHolder(ItemMarkdownToolbarCategoryBinding.inflate(inflater, parent, false))
                    } else {
                        ActionViewHolder(ItemMarkdownToolbarActionBinding.inflate(inflater, parent, false))
                    }

                @SuppressLint("ClickableViewAccessibility")
                override fun onBindViewHolder(
                    holder: RecyclerView.ViewHolder,
                    position: Int,
                ) {
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
                    handle.contentDescription =
                        context.getString(R.string.markdown_toolbar_drag_action_to_reorder, action.label)
                    handle.setOnTouchListener { _, event ->
                        if (event.actionMasked == MotionEvent.ACTION_DOWN) touchHelper.startDrag(holder)
                        false
                    }
                }
            }
        list.adapter = adapter
        touchHelper =
            ItemTouchHelper(
                object : ItemTouchHelper.SimpleCallback(
                    ItemTouchHelper.UP or ItemTouchHelper.DOWN,
                    0,
                ) {
                    override fun isLongPressDragEnabled() = false

                    override fun getMovementFlags(
                        recyclerView: RecyclerView,
                        viewHolder: RecyclerView.ViewHolder,
                    ): Int {
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

                    override fun onSwiped(
                        viewHolder: RecyclerView.ViewHolder,
                        direction: Int,
                    ) = Unit

                    override fun clearView(
                        recyclerView: RecyclerView,
                        viewHolder: RecyclerView.ViewHolder,
                    ) {
                        super.clearView(recyclerView, viewHolder)
                        save()
                        rebuild()
                    }
                },
            )
        touchHelper.attachToRecyclerView(list)
        val dialog =
            MaterialAlertDialogBuilder(context, R.style.AppTheme_AlertDialog)
                .setView(dialogBinding.root)
                .create()
        dialogBinding.restoreDefaultsButton.setOnClickListener {
            orderedActions = actions.toMutableList()
            enabledActions = defaultActions.toMutableSet()
            save()
            rebuild()
            populateRows()
            adapter.notifyItemRangeChanged(0, rows.size)
        }
        dialogBinding.doneButton.setOnClickListener { dialog.dismiss() }
        dialog.show()
    }
}
