package app.lockbook.ui

import android.content.res.ColorStateList
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.isVisible
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.CreateFileFolderItemBinding
import app.lockbook.model.FolderChoice
import com.google.android.material.color.MaterialColors

class CreateFolderAdapter(
    private val onClick: (FolderChoice) -> Unit,
    private val onToggle: (FolderChoice) -> Unit,
) : ListAdapter<FolderChoice, CreateFolderAdapter.ViewHolder>(Diff) {
    var selectedId: String? = null
        set(value) {
            val old = field
            field = value
            currentList.indexOfFirst { it.file.id == old }.takeIf { it >= 0 }?.let(::notifyItemChanged)
            currentList.indexOfFirst { it.file.id == value }.takeIf { it >= 0 }?.let(::notifyItemChanged)
        }

    var showHierarchy: Boolean = true
        set(value) {
            if (field == value) return
            field = value
            notifyItemRangeChanged(0, itemCount)
        }

    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): ViewHolder =
        ViewHolder(
            CreateFileFolderItemBinding.inflate(LayoutInflater.from(parent.context), parent, false),
        )

    override fun onBindViewHolder(
        holder: ViewHolder,
        position: Int,
    ) {
        holder.bind(getItem(position))
    }

    inner class ViewHolder(
        private val binding: CreateFileFolderItemBinding,
    ) : RecyclerView.ViewHolder(binding.root) {
        fun bind(choice: FolderChoice) {
            val selected = choice.file.id == selectedId
            binding.folderName.text = if (choice.file.isRoot) binding.root.context.getString(R.string.home) else choice.file.name
            binding.folderPath.text = choice.path
            binding.folderPath.isVisible = !showHierarchy
            binding.folderIcon.setImageResource(
                if (choice.file.type == net.lockbook.File.FileType.Folder) {
                    R.drawable.ic_baseline_folder_24
                } else {
                    R.drawable.ic_outline_insert_drive_file_24
                },
            )
            val density = binding.root.resources.displayMetrics.density
            binding.folderContent.setPadding(
                ((16 + if (showHierarchy) minOf(choice.depth, MAX_INDENT_DEPTH) * 20 else 0) * density).toInt(),
                binding.folderContent.paddingTop,
                binding.folderContent.paddingRight,
                binding.folderContent.paddingBottom,
            )
            val isFolder = choice.file.type == net.lockbook.File.FileType.Folder
            binding.folderExpand.isVisible = showHierarchy && isFolder && choice.hasChildren
            binding.folderExpand.rotation = if (choice.isExpanded) 90f else 0f
            binding.folderExpand.setOnClickListener(
                if (isFolder && choice.hasChildren) View.OnClickListener { onToggle(choice) } else null,
            )
            binding.root.alpha = if (isFolder) 1f else 0.4f
            binding.root.isClickable = isFolder
            binding.root.isFocusable = isFolder
            binding.root.setCardBackgroundColor(
                ColorStateList.valueOf(
                    MaterialColors.getColor(
                        binding.root,
                        if (selected) {
                            com.google.android.material.R.attr.colorSecondaryContainer
                        } else {
                            com.google.android.material.R.attr.colorSurfaceContainerLow
                        },
                    ),
                ),
            )
            binding.root.setOnClickListener(if (isFolder) View.OnClickListener { onClick(choice) } else null)
        }
    }

    private object Diff : DiffUtil.ItemCallback<FolderChoice>() {
        override fun areItemsTheSame(
            oldItem: FolderChoice,
            newItem: FolderChoice,
        ): Boolean = oldItem.file.id == newItem.file.id

        override fun areContentsTheSame(
            oldItem: FolderChoice,
            newItem: FolderChoice,
        ): Boolean = oldItem == newItem
    }

    private companion object {
        const val MAX_INDENT_DEPTH = 3
    }
}
