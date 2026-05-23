package app.lockbook.screen

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import androidx.constraintlayout.widget.ConstraintLayout
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.util.DocumentViewHolder
import app.lockbook.util.FileViewHolderInfo
import app.lockbook.util.FolderViewHolder
import app.lockbook.util.getIconResource
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

    override fun getItemViewType(position: Int): Int =
        when (getItem(position)) {
            is FileViewHolderInfo.FolderViewHolderInfo -> VIEW_TYPE_FOLDER
            is FileViewHolderInfo.DocumentViewHolderInfo -> VIEW_TYPE_DOCUMENT
        }

    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): RecyclerView.ViewHolder {
        val inflater = LayoutInflater.from(parent.context)
        return when (viewType) {
            VIEW_TYPE_FOLDER -> FolderViewHolder(inflater.inflate(R.layout.folder_file_item, parent, false))
            VIEW_TYPE_DOCUMENT -> DocumentViewHolder(inflater.inflate(R.layout.document_file_item, parent, false))
            else -> error("Unsupported view type $viewType")
        }
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
    ) {
        bind(holder, getItem(position))
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
        payloads: MutableList<Any>,
    ) {
        val item = getItem(position)
        if (payloads.contains(PAYLOAD_SELECTION)) {
            when (holder) {
                is FolderViewHolder -> {
                    bindSelectionState(
                        item as FileViewHolderInfo.FolderViewHolderInfo,
                        holder.fileItemHolder,
                        holder.actionIcon,
                    )
                }

                is DocumentViewHolder -> {
                    bindSelectionState(
                        item as FileViewHolderInfo.DocumentViewHolderInfo,
                        holder.fileItemHolder,
                        holder.actionIcon,
                    )
                }
            }
            return
        }

        bind(holder, item)
    }

    private fun bind(
        holder: RecyclerView.ViewHolder,
        item: FileViewHolderInfo,
    ) {
        holder.itemView.setOnClickListener { onItemClick(item) }
        holder.itemView.setOnLongClickListener {
            onItemLongClick(item)
            true
        }

        when (holder) {
            is FolderViewHolder -> bindFolder(holder, item as FileViewHolderInfo.FolderViewHolderInfo)
            is DocumentViewHolder -> bindDocument(holder, item as FileViewHolderInfo.DocumentViewHolderInfo)
        }
    }

    private fun bindFolder(
        holder: FolderViewHolder,
        item: FileViewHolderInfo.FolderViewHolderInfo,
    ) {
        holder.name.text = item.fileMetadata.name
        bindSelectionState(item, holder.fileItemHolder, holder.actionIcon)
    }

    private fun bindDocument(
        holder: DocumentViewHolder,
        item: FileViewHolderInfo.DocumentViewHolderInfo,
    ) {
        holder.name.text = item.fileMetadata.getPrettyName()
        if (item.fileMetadata.lastModified != 0L) {
            holder.description.visibility = View.VISIBLE
            holder.description.text = Lb.getTimestampHumanString(item.fileMetadata.lastModified)
        } else {
            holder.description.visibility = View.GONE
        }

        holder.icon.setImageResource(item.fileMetadata.getIconResource())
        bindSelectionState(item, holder.fileItemHolder, holder.actionIcon)
    }

    private fun bindSelectionState(
        item: FileViewHolderInfo,
        fileItemHolder: ConstraintLayout,
        actionIcon: ImageView,
    ) {
        val isSelected = selectedFileIds.contains(item.fileMetadata.id)
        fileItemHolder.isSelected = isSelected

        when {
            item.needsToBePulled -> {
                actionIcon.setImageResource(R.drawable.ic_baseline_cloud_download_24)
                actionIcon.visibility = View.VISIBLE
            }

            item.needToBePushed -> {
                actionIcon.setImageResource(R.drawable.ic_baseline_cloud_upload_24)
                actionIcon.visibility = View.VISIBLE
            }

            item.isShared -> {
                actionIcon.setImageResource(R.drawable.ic_baseline_group_24)
                actionIcon.visibility = View.VISIBLE
            }

            else -> {
                actionIcon.visibility = View.GONE
            }
        }

        if (isSelected) {
            actionIcon.setImageResource(R.drawable.ic_baseline_check_small_24)
            actionIcon.visibility = View.VISIBLE
        }
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

private const val VIEW_TYPE_FOLDER = 1
private const val VIEW_TYPE_DOCUMENT = 2
private const val PAYLOAD_SELECTION = "selection"
