package app.lockbook.screen

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.util.FileMetadataRowInfo
import app.lockbook.util.FileMetadataStatusIcon
import app.lockbook.util.FileMetadataViewHolder
import app.lockbook.util.FileViewHolderInfo
import app.lockbook.util.getIconResource
import com.google.android.material.listitem.ListItemLayout
import net.lockbook.Lb

class FileTreeAdapter(
    private val onItemClick: (FileViewHolderInfo) -> Unit,
    private val onItemLongClick: (FileViewHolderInfo) -> Unit,
) : ListAdapter<FileViewHolderInfo, RecyclerView.ViewHolder>(FileTreeDiffCallback()) {
    private var selectedFileIds: Set<String> = emptySet()

    fun setSelectedFileIds(newSelection: Set<String>) {
        if (selectedFileIds == newSelection) {
            return
        }

        val oldSelection = selectedFileIds
        selectedFileIds = newSelection.toSet()

        currentList.forEachIndexed { index, item ->
            val id = item.fileMetadata.id
            val changed = oldSelection.contains(id) != selectedFileIds.contains(id)
            if (changed) {
                notifyItemChanged(index, PAYLOAD_SELECTION)
            }
        }
    }

    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): RecyclerView.ViewHolder {
        val inflater = LayoutInflater.from(parent.context)
        return FileMetadataViewHolder(inflater.inflate(R.layout.file_metadata_item, parent, false))
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
    ) {
        bind(holder, getItem(position), position)
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
        payloads: MutableList<Any>,
    ) {
        val item = getItem(position)
        if (payloads.contains(PAYLOAD_SELECTION)) {
            bind(holder, item, position)
            return
        }

        bind(holder, item, position)
    }

    private fun bind(
        holder: RecyclerView.ViewHolder,
        item: FileViewHolderInfo,
        position: Int,
    ) {
        updateListItemAppearance(holder, position)

        when (holder) {
            is FileMetadataViewHolder -> holder.bind(item.toFileMetadataRowInfo())
        }

        val clickTarget =
            when (holder) {
                is FileMetadataViewHolder -> holder.fileItemHolder
                else -> holder.itemView
            }

        clickTarget.setOnClickListener { onItemClick(item) }
        clickTarget.setOnLongClickListener {
            onItemLongClick(item)
            true
        }
    }

    private fun updateListItemAppearance(
        holder: RecyclerView.ViewHolder,
        position: Int,
    ) {
        (holder.itemView as? ListItemLayout)?.updateAppearance(position, itemCount)
    }

    private fun FileViewHolderInfo.toFileMetadataRowInfo(): FileMetadataRowInfo {
        val isSelected = selectedFileIds.contains(fileMetadata.id)
        val statusIcon =
            when {
                needsToBePulled -> {
                    FileMetadataStatusIcon(R.drawable.ic_baseline_cloud_download_24)
                }

                needToBePushed -> {
                    FileMetadataStatusIcon(R.drawable.ic_baseline_cloud_upload_24)
                }

                isShared -> {
                    FileMetadataStatusIcon(R.drawable.ic_baseline_group_24)
                }

                else -> {
                    null
                }
            }

        return FileMetadataRowInfo(
            file = fileMetadata,
            title =
                when (this) {
                    is FileViewHolderInfo.DocumentViewHolderInfo -> fileMetadata.getPrettyName()
                    is FileViewHolderInfo.FolderViewHolderInfo -> fileMetadata.name
                },
            subtitle =
                if (this is FileViewHolderInfo.DocumentViewHolderInfo && fileMetadata.lastModified != 0L) {
                    Lb.getTimestampHumanString(fileMetadata.lastModified)
                } else {
                    null
                },
            iconRes = fileMetadata.getIconResource(),
            statusIcon = if (isSelected) FileMetadataStatusIcon(R.drawable.ic_baseline_check_small_24) else statusIcon,
            isSelected = isSelected,
            isChecked = isSelected,
        )
    }
}

private class FileTreeDiffCallback : DiffUtil.ItemCallback<FileViewHolderInfo>() {
    override fun areItemsTheSame(
        oldItem: FileViewHolderInfo,
        newItem: FileViewHolderInfo,
    ): Boolean = oldItem.fileMetadata.id == newItem.fileMetadata.id

    override fun areContentsTheSame(
        oldItem: FileViewHolderInfo,
        newItem: FileViewHolderInfo,
    ): Boolean = oldItem == newItem
}

private const val PAYLOAD_SELECTION = "selection"
