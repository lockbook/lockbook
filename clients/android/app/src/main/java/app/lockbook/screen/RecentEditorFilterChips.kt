package app.lockbook.screen

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.doOnLayout
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.RecentEditorFiltersBinding
import app.lockbook.model.LastEditorFilterState
import app.lockbook.model.LastEditorFilterViewModel
import app.lockbook.ui.UsernameChip
import com.google.android.material.chip.Chip
import com.google.android.material.chip.ChipGroup
import net.lockbook.File

/** A list header so editor filters scroll vertically with the date groups. */
internal class RecentEditorFilterAdapter(
    private val model: LastEditorFilterViewModel,
) : RecyclerView.Adapter<RecentEditorFilterAdapter.Holder>() {
    private var state = LastEditorFilterState()
    private var horizontalOffset = 0
    private var usernames: List<String> = emptyList()
    private val visible get() = state.isVisible && usernames.isNotEmpty()

    fun submitState(
        updated: LastEditorFilterState,
        files: List<File>,
    ) {
        val updatedUsernames = updated.recentUsernames(files)
        if (state == updated && usernames == updatedUsernames) return
        val wasVisible = visible
        state = updated
        usernames = updatedUsernames
        when {
            !wasVisible && visible -> notifyItemInserted(0)
            wasVisible && !visible -> notifyItemRemoved(0)
            visible -> notifyItemChanged(0)
        }
    }

    override fun getItemCount(): Int = if (visible) 1 else 0

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
        holder.chips.render(state, usernames)
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
    fun render(
        state: LastEditorFilterState,
        editors: List<String>,
    ) {
        val group = binding.recentEditorChips
        val usernames = (0 until group.childCount).map { group.getChildAt(it).tag }
        if (usernames != editors) {
            group.removeAllViews()
            editors.forEach { username ->
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
): UsernameChip =
    (LayoutInflater.from(group.context).inflate(R.layout.last_editor_filter_chip, group, false) as UsernameChip).apply {
        id = View.generateViewId()
        bindUsername(username)
    }
