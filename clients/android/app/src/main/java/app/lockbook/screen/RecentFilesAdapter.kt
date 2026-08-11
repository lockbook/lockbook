package app.lockbook.screen

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.TextView
import androidx.core.content.ContextCompat
import androidx.core.view.isVisible
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.model.RecentPeriod
import app.lockbook.model.groupRecentDocuments
import app.lockbook.model.recentFileParentPath
import app.lockbook.util.getIconResource
import com.google.android.material.chip.Chip
import com.google.android.material.listitem.ListItemLayout
import net.lockbook.File
import net.lockbook.Lb

internal sealed interface RecentListItem {
    data class Section(
        val period: RecentPeriod,
        val collapsed: Boolean,
    ) : RecentListItem

    data class Document(
        val file: File,
        val parentPath: String,
        val positionInSection: Int,
        val sectionSize: Int,
    ) : RecentListItem
}

internal class RecentFilesAdapter(
    private val onFileClick: (File) -> Unit,
    private val onFileLongClick: (File) -> Unit,
    private val currentUsername: String?,
) : ListAdapter<RecentListItem, RecyclerView.ViewHolder>(RecentListItemDiffCallback()) {
    private var files: Collection<File> = emptyList()
    private var filesById: Map<String, File> = emptyMap()
    private val collapsedPeriods = mutableSetOf<RecentPeriod>()
    private var selectedFileIds: Set<String> = emptySet()

    fun setSelectedFileIds(newSelection: Set<String>) {
        if (selectedFileIds == newSelection) return
        val oldSelection = selectedFileIds
        selectedFileIds = newSelection.toSet()
        currentList.forEachIndexed { index, item ->
            if (item is RecentListItem.Document &&
                oldSelection.contains(item.file.id) != selectedFileIds.contains(item.file.id)
            ) {
                notifyItemChanged(index, PAYLOAD_SELECTION)
            }
        }
    }

    fun submitRecentFiles(
        files: Collection<File>,
        filesById: Map<String, File>,
    ) {
        this.files = files
        this.filesById = filesById
        rebuildList()
    }

    override fun getItemViewType(position: Int): Int =
        when (getItem(position)) {
            is RecentListItem.Section -> VIEW_TYPE_SECTION
            is RecentListItem.Document -> VIEW_TYPE_DOCUMENT
        }

    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): RecyclerView.ViewHolder {
        val inflater = LayoutInflater.from(parent.context)
        return when (viewType) {
            VIEW_TYPE_SECTION -> {
                DateSectionViewHolder(
                    inflater.inflate(R.layout.file_date_section_item, parent, false),
                )
            }

            else -> {
                RecentDocumentViewHolder(
                    inflater.inflate(R.layout.recent_file_item, parent, false),
                )
            }
        }
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
    ) {
        when (val item = getItem(position)) {
            is RecentListItem.Section -> {
                (holder as DateSectionViewHolder).bind(item.period, item.collapsed) {
                    if (selectedFileIds.isNotEmpty()) return@bind
                    if (!collapsedPeriods.add(item.period)) {
                        collapsedPeriods.remove(item.period)
                    }
                    rebuildList()
                }
            }

            is RecentListItem.Document -> {
                bindDocument(holder as RecentDocumentViewHolder, item)
            }
        }
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
        payloads: MutableList<Any>,
    ) {
        val item = getItem(position)
        if (payloads.contains(PAYLOAD_SELECTION) && item is RecentListItem.Document) {
            bindDocument(holder as RecentDocumentViewHolder, item)
        } else {
            onBindViewHolder(holder, position)
        }
    }

    private fun bindDocument(
        holder: RecentDocumentViewHolder,
        item: RecentListItem.Document,
    ) {
        holder.bind(
            item = item,
            currentUsername = currentUsername,
            selected = item.file.id in selectedFileIds,
            onClick = onFileClick,
            onLongClick = onFileLongClick,
        )
    }

    private fun rebuildList() {
        val items =
            buildList {
                groupRecentDocuments(files).forEach { group ->
                    val collapsed = group.period in collapsedPeriods
                    add(RecentListItem.Section(group.period, collapsed))
                    if (!collapsed) {
                        group.files.forEachIndexed { index, file ->
                            add(
                                RecentListItem.Document(
                                    file = file,
                                    parentPath = recentFileParentPath(file, filesById),
                                    positionInSection = index,
                                    sectionSize = group.files.size,
                                ),
                            )
                        }
                    }
                }
            }
        submitList(items)
    }

    private companion object {
        const val VIEW_TYPE_SECTION = 0
        const val VIEW_TYPE_DOCUMENT = 1
        const val PAYLOAD_SELECTION = "selection"
    }
}

private class RecentDocumentViewHolder(
    itemView: View,
) : RecyclerView.ViewHolder(itemView) {
    private val control: com.google.android.material.listitem.ListItemCardView =
        itemView.findViewById(R.id.recent_file_control)
    private val icon: ImageView = itemView.findViewById(R.id.recent_file_icon)
    private val selectedBadge: ImageView = itemView.findViewById(R.id.recent_file_selected_badge)
    private val title: TextView = itemView.findViewById(R.id.recent_file_title)
    private val modified: TextView = itemView.findViewById(R.id.recent_file_modified)
    private val path: TextView = itemView.findViewById(R.id.recent_file_path)
    private val editor: Chip = itemView.findViewById(R.id.recent_file_editor)

    fun bind(
        item: RecentListItem.Document,
        currentUsername: String?,
        selected: Boolean,
        onClick: (File) -> Unit,
        onLongClick: (File) -> Unit,
    ) {
        icon.setImageResource(item.file.getIconResource())
        title.text = item.file.name
        modified.text = Lb.getTimestampHumanString(item.file.lastModified)
        path.text = item.parentPath
        val showEditor =
            currentUsername != null &&
                item.file.lastModifiedBy.isNotBlank() &&
                item.file.lastModifiedBy != currentUsername
        editor.isVisible = showEditor
        editor.text = item.file.lastModifiedBy
        control.isSelected = selected
        control.isChecked = selected
        control.setCardBackgroundColor(ContextCompat.getColorStateList(itemView.context, R.color.file_tree_list_item_background))
        selectedBadge.isVisible = selected
        (itemView as ListItemLayout).updateAppearance(item.positionInSection, item.sectionSize)
        control.setOnClickListener { onClick(item.file) }
        control.setOnLongClickListener {
            onLongClick(item.file)
            true
        }
    }
}

private class RecentListItemDiffCallback : DiffUtil.ItemCallback<RecentListItem>() {
    override fun areItemsTheSame(
        oldItem: RecentListItem,
        newItem: RecentListItem,
    ): Boolean =
        when {
            oldItem is RecentListItem.Section && newItem is RecentListItem.Section -> oldItem.period == newItem.period
            oldItem is RecentListItem.Document && newItem is RecentListItem.Document -> oldItem.file.id == newItem.file.id
            else -> false
        }

    override fun areContentsTheSame(
        oldItem: RecentListItem,
        newItem: RecentListItem,
    ): Boolean = oldItem == newItem
}
