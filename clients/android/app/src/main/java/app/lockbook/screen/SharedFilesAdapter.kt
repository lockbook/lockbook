package app.lockbook.screen

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.model.RecentPeriod
import app.lockbook.util.FileMetadataRowInfo
import app.lockbook.util.FileMetadataTrailingButton
import app.lockbook.util.FileMetadataViewHolder
import app.lockbook.util.getIconResource
import com.google.android.material.listitem.ListItemLayout
import net.lockbook.File

internal sealed interface SharedListItem {
    data class Section(
        val period: RecentPeriod,
        val collapsed: Boolean,
    ) : SharedListItem

    data class FileItem(
        val file: File,
        val subtitle: String? = null,
        val showPendingMenu: Boolean = false,
        val positionInSection: Int,
        val sectionSize: Int,
    ) : SharedListItem
}

internal class SharedFilesAdapter(
    private val onFileClick: (File) -> Unit,
    private val onPendingMenuClick: (View, File) -> Unit,
) : ListAdapter<SharedListItem, RecyclerView.ViewHolder>(SharedListItemDiffCallback()) {
    private var sourceItems: List<SharedListItem> = emptyList()
    private val collapsedPeriods = mutableSetOf<RecentPeriod>()

    fun submitSharedFiles(items: List<SharedListItem>) {
        sourceItems = items
        rebuildList()
    }

    override fun getItemViewType(position: Int): Int =
        when (getItem(position)) {
            is SharedListItem.Section -> VIEW_TYPE_SECTION
            is SharedListItem.FileItem -> VIEW_TYPE_FILE
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
                FileMetadataViewHolder(
                    inflater.inflate(R.layout.file_metadata_item, parent, false),
                )
            }
        }
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
    ) {
        when (val item = getItem(position)) {
            is SharedListItem.Section -> {
                (holder as DateSectionViewHolder).bind(item.period, item.collapsed) {
                    if (!collapsedPeriods.add(item.period)) {
                        collapsedPeriods.remove(item.period)
                    }
                    rebuildList()
                }
            }

            is SharedListItem.FileItem -> {
                bindFile(holder as FileMetadataViewHolder, item, position)
            }
        }
    }

    private fun bindFile(
        holder: FileMetadataViewHolder,
        item: SharedListItem.FileItem,
        position: Int,
    ) {
        holder.bind(
            FileMetadataRowInfo(
                file = item.file,
                title = item.file.name,
                subtitle = item.subtitle,
                iconRes = item.file.getIconResource(),
                trailingButton =
                    if (item.showPendingMenu) {
                        FileMetadataTrailingButton(
                            iconRes = R.drawable.ic_baseline_more_vert_24,
                            contentDescriptionRes = R.string.open_pending_share_menu,
                        ) { view -> onPendingMenuClick(view, item.file) }
                    } else {
                        null
                    },
            ),
        )
        holder.fileItemHolder.setOnClickListener { onFileClick(item.file) }

        (holder.itemView as ListItemLayout).updateAppearance(item.positionInSection, item.sectionSize)
    }

    private fun rebuildList() {
        var collapsed = false
        submitList(
            sourceItems.mapNotNull { item ->
                when (item) {
                    is SharedListItem.Section -> {
                        collapsed = item.period in collapsedPeriods
                        item.copy(collapsed = collapsed)
                    }

                    is SharedListItem.FileItem -> {
                        item.takeUnless { collapsed }
                    }
                }
            },
        )
    }

    private companion object {
        const val VIEW_TYPE_SECTION = 0
        const val VIEW_TYPE_FILE = 1
    }
}

private class SharedListItemDiffCallback : DiffUtil.ItemCallback<SharedListItem>() {
    override fun areItemsTheSame(
        oldItem: SharedListItem,
        newItem: SharedListItem,
    ): Boolean =
        when {
            oldItem is SharedListItem.Section && newItem is SharedListItem.Section -> oldItem.period == newItem.period
            oldItem is SharedListItem.FileItem && newItem is SharedListItem.FileItem -> oldItem.file.id == newItem.file.id
            else -> false
        }

    override fun areContentsTheSame(
        oldItem: SharedListItem,
        newItem: SharedListItem,
    ): Boolean = oldItem == newItem
}
