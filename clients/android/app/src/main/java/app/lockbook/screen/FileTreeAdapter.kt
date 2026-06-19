package app.lockbook.screen

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.util.DocumentViewHolder
import app.lockbook.util.FileViewHolderInfo
import app.lockbook.util.getIconResource
import com.google.android.material.listitem.ListItemCardView
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
        return DocumentViewHolder(inflater.inflate(R.layout.document_file_item, parent, false))
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
            updateListItemAppearance(holder, position)
            if (holder is DocumentViewHolder) {
                bindSelectionState(item, holder.fileItemHolder, holder.actionIcon)
            }
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

        val clickTarget = when (holder) {
            is DocumentViewHolder -> holder.fileItemHolder
            else -> holder.itemView
        }

        clickTarget.setOnClickListener { onItemClick(item) }
        clickTarget.setOnLongClickListener {
            onItemLongClick(item)
            true
        }

        when (holder) {
            is DocumentViewHolder -> bindFile(holder, item)
        }
    }

    private fun bindFile(
        holder: DocumentViewHolder,
        item: FileViewHolderInfo,
    ) {
        holder.name.text =
            when (item) {
                is FileViewHolderInfo.DocumentViewHolderInfo -> item.fileMetadata.getPrettyName()
                is FileViewHolderInfo.FolderViewHolderInfo -> item.fileMetadata.name
            }

        if (item is FileViewHolderInfo.DocumentViewHolderInfo && item.fileMetadata.lastModified != 0L) {
            holder.description.visibility = View.VISIBLE
            holder.description.text = Lb.getTimestampHumanString(item.fileMetadata.lastModified)
        } else {
            holder.description.visibility = View.GONE
        }

        holder.icon.setImageResource(item.fileMetadata.getIconResource())
        bindSelectionState(item, holder.fileItemHolder, holder.actionIcon)
    }

    private fun updateListItemAppearance(
        holder: RecyclerView.ViewHolder,
        position: Int,
    ) {
        (holder.itemView as? ListItemLayout)?.updateAppearance(position, itemCount)
    }

    private fun bindSelectionState(
        item: FileViewHolderInfo,
        fileItemHolder: ListItemCardView,
        actionIcon: ImageView,
    ) {
        val isSelected = selectedFileIds.contains(item.fileMetadata.id)
        fileItemHolder.isSelected = isSelected
        fileItemHolder.isChecked = isSelected

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

private const val PAYLOAD_SELECTION = "selection"
