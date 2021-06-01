package app.lockbook.model

import android.widget.ImageView
import androidx.cardview.widget.CardView
import androidx.core.content.res.ResourcesCompat
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.FileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.ListFilesClickInterface

abstract class GeneralViewAdapter(val listFilesClickInterface: ListFilesClickInterface) : RecyclerView.Adapter<GeneralViewAdapter.FileViewHolder>() {
    abstract var files: List<FileMetadata>
    abstract var selectedFiles: MutableList<Boolean>

    inner class FileViewHolder(val cardView: CardView) : RecyclerView.ViewHolder(cardView) {
        lateinit var fileMetadata: FileMetadata

        init {
            cardView.setOnClickListener {
                if (selectedFiles.contains(true)) {
                    changeItemBasedOnSelection()
                    listFilesClickInterface.onItemClick(adapterPosition, true, selectedFiles)
                } else {
                    listFilesClickInterface.onItemClick(adapterPosition, false, selectedFiles)
                }
            }

            cardView.setOnLongClickListener {
                if (!selectedFiles.contains(true)) {
                    changeItemBasedOnSelection()
                    listFilesClickInterface.onLongClick(adapterPosition, selectedFiles)
                }
                true
            }
        }

        private fun changeItemBasedOnSelection() {
            selectedFiles[adapterPosition] = !selectedFiles[adapterPosition]
            val fileIcon = cardView.findViewById<ImageView>(R.id.linear_file_icon)
            val gridIcon = cardView.findViewById<ImageView>(R.id.grid_file_icon)

            if (selectedFiles[adapterPosition]) {
                cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.selectedFileBackground,
                        App.instance.theme
                    )
                )

                if (fileIcon != null) {
                    fileIcon.setImageResource(R.drawable.ic_baseline_check_24)
                } else {
                    gridIcon.setImageResource(R.drawable.ic_baseline_check_24)
                }
            } else {
                if (fileMetadata.fileType == FileType.Document && fileMetadata.name.endsWith(".draw")) {
                    if (fileIcon != null) {
                        fileIcon.setImageResource(R.drawable.ic_baseline_border_color_24)
                    } else {
                        gridIcon.setImageResource(R.drawable.ic_baseline_border_color_24)
                    }
                } else if (fileMetadata.fileType == FileType.Document) {
                    if (fileIcon != null) {
                        fileIcon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                    } else {
                        gridIcon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                    }
                } else {
                    if (fileIcon != null) {
                        fileIcon.setImageResource(R.drawable.round_folder_white_18dp)
                    } else {
                        gridIcon.setImageResource(R.drawable.round_folder_white_18dp)
                    }
                }
                cardView.background.setTintList(null)
                cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
            }
        }
    }
}
