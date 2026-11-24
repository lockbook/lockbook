package app.lockbook.screen

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.doOnLayout
import androidx.core.view.isVisible
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.RecentEditorFiltersBinding
import app.lockbook.model.RecentDocumentType
import app.lockbook.model.RecentsFilterState
import app.lockbook.model.RecentsFilterViewModel
import app.lockbook.ui.UsernameChip
import com.google.android.material.chip.Chip
import com.google.android.material.chip.ChipGroup
import net.lockbook.File

/** A list header so editor filters scroll vertically with the date groups. */
internal class RecentEditorFilterAdapter(
    private val model: RecentsFilterViewModel,
) : RecyclerView.Adapter<RecentEditorFilterAdapter.Holder>() {
    private var state = RecentsFilterState()
    private var horizontalOffset = 0
    private var usernames: List<String> = emptyList()

    fun submitState(
        updated: RecentsFilterState,
        files: List<File>,
    ) {
        val updatedUsernames = if (updated.editorFilter.hasMultipleEditors) updated.editorFilter.recentUsernames(files) else emptyList()
        if (state == updated && usernames == updatedUsernames) return
        state = updated
        usernames = updatedUsernames
        notifyItemChanged(0)
    }

    override fun getItemCount(): Int = 1

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
        model: RecentsFilterViewModel,
    ) : RecyclerView.ViewHolder(binding.root) {
        val chips = RecentEditorFilterChips(binding, model)
    }
}

/** All editors stay in the row, with Material's checked state indicating included files. */
internal class RecentEditorFilterChips(
    private val binding: RecentEditorFiltersBinding,
    private val model: RecentsFilterViewModel,
) {
    init {
        binding.recentFilterNotes.setOnClickListener { model.toggleDocumentType(RecentDocumentType.Notes) }
        binding.recentFilterDrawings.setOnClickListener { model.toggleDocumentType(RecentDocumentType.Drawings) }
        binding.recentFilterImages.setOnClickListener { model.toggleDocumentType(RecentDocumentType.Images) }
    }

    fun render(
        state: RecentsFilterState,
        editors: List<String>,
    ) {
        for ((chip, type) in listOf(
            binding.recentFilterNotes to RecentDocumentType.Notes,
            binding.recentFilterDrawings to RecentDocumentType.Drawings,
            binding.recentFilterImages to RecentDocumentType.Images,
        )) {
            chip.isChecked = type in state.documentTypes
            chip.isChipIconVisible = !chip.isChecked
        }
        binding.filterCategoryDivider.isVisible = editors.isNotEmpty()
        binding.recentEditorChips.isVisible = editors.isNotEmpty()
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
            val selected = state.editorFilter.selected == null || chip.tag in state.editorFilter.selected
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
