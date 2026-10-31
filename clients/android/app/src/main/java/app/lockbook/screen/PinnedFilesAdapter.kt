package app.lockbook.screen

import android.content.res.Configuration
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.model.PinnedFile
import app.lockbook.util.Previews
import com.google.android.material.textview.MaterialTextView
import net.lockbook.File

data class PinnedFileItem(
    val pin: PinnedFile,
    val file: File,
)

class PinnedFilesAdapter(
    private val onItemClick: (PinnedFileItem) -> Unit,
    private val onItemLongClick: (PinnedFileItem) -> Unit,
) : ListAdapter<PinnedFileItem, PinnedFilesAdapter.PinnedFileViewHolder>(PinnedFilesDiffCallback()) {
    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): PinnedFileViewHolder =
        PinnedFileViewHolder(
            LayoutInflater.from(parent.context).inflate(R.layout.pinned_file_item, parent, false),
        )

    override fun onBindViewHolder(
        holder: PinnedFileViewHolder,
        position: Int,
    ) {
        val item = getItem(position)
        holder.title.text = item.file.name
        holder.emoji.text = item.pin.emoji
        holder.emoji.visibility = if (item.pin.emoji == null) View.GONE else View.VISIBLE
        val night = holder.itemView.resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
        val preview = Previews.get(item.file.id, night == Configuration.UI_MODE_NIGHT_YES)
        holder.preview.setImageBitmap(preview)
        holder.preview.visibility = if (preview == null) View.GONE else View.VISIBLE
        holder.itemView.setOnClickListener { onItemClick(item) }
        holder.itemView.setOnLongClickListener {
            onItemLongClick(item)
            true
        }
    }

    class PinnedFileViewHolder(
        itemView: View,
    ) : RecyclerView.ViewHolder(itemView) {
        val emoji: MaterialTextView = itemView.findViewById(R.id.pinned_file_emoji)
        val title: MaterialTextView = itemView.findViewById(R.id.pinned_file_title)
        val preview: ImageView = itemView.findViewById(R.id.pinned_file_preview)
    }
}

private class PinnedFilesDiffCallback : DiffUtil.ItemCallback<PinnedFileItem>() {
    override fun areItemsTheSame(
        oldItem: PinnedFileItem,
        newItem: PinnedFileItem,
    ): Boolean = oldItem.pin.id == newItem.pin.id

    override fun areContentsTheSame(
        oldItem: PinnedFileItem,
        newItem: PinnedFileItem,
    ): Boolean = oldItem == newItem
}
