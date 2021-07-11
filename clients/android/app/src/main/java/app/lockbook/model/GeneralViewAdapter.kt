package app.lockbook.model

import android.widget.ImageView
import androidx.cardview.widget.CardView
import androidx.core.content.res.ResourcesCompat
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.util.ClientFileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.ListFilesClickInterface

abstract class GeneralViewAdapter(val listFilesClickInterface: ListFilesClickInterface) : RecyclerView.Adapter<GeneralViewAdapter.FileViewHolder>() {
    abstract var files: List<ClientFileMetadata>
    abstract var selectedFiles: MutableList<ClientFileMetadata>
    abstract var selectionMode: Boolean

    fun enterSelectionModeWithItem(selectedItemPosition: Int) {
        if (selectedItemPosition >= 0 && selectedItemPosition < files.size) {
            selectedFiles.add(files[selectedItemPosition])
        }
        selectionMode = true
        notifyDataSetChanged()
    }

    fun clearSelectionMode() {
        selectionMode = false
        selectedFiles.clear()
        notifyDataSetChanged()
    }

    inner class FileViewHolder(val cardView: CardView) : RecyclerView.ViewHolder(cardView) {
        lateinit var fileMetadata: ClientFileMetadata

        init {
            cardView.setOnClickListener {
                val selectedItem = files[adapterPosition]

                if (selectionMode) {
                    val isFileAlreadySelected = selectedFiles.contains(selectedItem)
                    changeItemBasedOnNewSelection(!isFileAlreadySelected)
                    if (isFileAlreadySelected) {
                        selectedFiles.remove(selectedItem)
                        if (selectedFiles.isEmpty()) {
                            selectionMode = false
                        }
                    } else {
                        selectedFiles.add(selectedItem)
                    }
                }

                listFilesClickInterface.onItemClick(adapterPosition, selectedFiles)
            }

            cardView.setOnLongClickListener {
                if (!selectionMode) {
                    enterSelectionModeWithItem(adapterPosition)
                    changeItemBasedOnNewSelection(true)
                }

                listFilesClickInterface.onLongClick(adapterPosition, selectedFiles)
                true
            }
        }

        private fun changeItemBasedOnNewSelection(isSelected: Boolean) {
            val fileIcon = cardView.findViewById<ImageView>(R.id.linear_file_icon)
            val gridIcon = cardView.findViewById<ImageView>(R.id.grid_file_icon)
            val icon = fileIcon ?: gridIcon

            if (isSelected) {
                icon.setImageResource(R.drawable.ic_baseline_check_24)
            } else {
                when {
                    fileMetadata.fileType == FileType.Document && fileMetadata.name.endsWith(".draw") -> {
                        icon.setImageResource(R.drawable.ic_baseline_border_color_24)
                    }
                    fileMetadata.fileType == FileType.Document -> icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                    else -> icon.setImageResource(R.drawable.round_folder_white_18dp)
                }
                cardView.background.setTintList(null)
            }

            cardView.background.setTint(
                ResourcesCompat.getColor(
                    cardView.resources,
                    if (isSelected) R.color.selectedFileBackground else R.color.colorPrimaryDark,
                    cardView.context.theme
                )
            )
        }
    }
}
