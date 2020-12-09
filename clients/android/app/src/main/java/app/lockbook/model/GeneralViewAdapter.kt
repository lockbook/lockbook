package app.lockbook.model

import androidx.cardview.widget.CardView
import androidx.core.content.res.ResourcesCompat
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.FileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.ListFilesClickInterface
import kotlinx.android.synthetic.main.grid_layout_file_item.view.*
import kotlinx.android.synthetic.main.linear_layout_file_item.view.*

abstract class GeneralViewAdapter(val listFilesClickInterface: ListFilesClickInterface) : RecyclerView.Adapter<GeneralViewAdapter.FileViewHolder>() {
    abstract var files: List<FileMetadata>
    abstract var selectedFiles: MutableList<Boolean>

    inner class FileViewHolder(val cardView: CardView) : RecyclerView.ViewHolder(cardView) {
        lateinit var fileMetadata: FileMetadata

        init {
            cardView.setOnClickListener {
                if (selectedFiles.contains(true)) {
                    setImageResourceBasedOnSelection()
                    listFilesClickInterface.onItemClick(adapterPosition, true, selectedFiles)
                } else {
                    listFilesClickInterface.onItemClick(adapterPosition, false, selectedFiles)
                }
            }

            cardView.setOnLongClickListener {
                if (!selectedFiles.contains(true)) {
                    setImageResourceBasedOnSelection()
                    listFilesClickInterface.onLongClick(adapterPosition, selectedFiles)
                }
                true
            }
        }

        private fun setImageResourceBasedOnSelection() {
            selectedFiles[adapterPosition] = !selectedFiles[adapterPosition]

            if (selectedFiles[adapterPosition]) {
                cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.selectedFileBackground,
                        App.instance.theme
                    )
                )
                if (cardView.linear_file_icon != null) {
                    cardView.linear_file_icon.setImageResource(R.drawable.ic_baseline_check_24)
                } else {
                    cardView.grid_file_icon.setImageResource(R.drawable.ic_baseline_check_24)
                }
            } else {
                if (fileMetadata.fileType == FileType.Document && fileMetadata.name.endsWith(".draw")) {
                    if (cardView.linear_file_icon != null) {
                        cardView.linear_file_icon.setImageResource(R.drawable.ic_baseline_border_color_24)
                    } else {
                        cardView.grid_file_icon.setImageResource(R.drawable.ic_baseline_border_color_24)
                    }
                } else if (fileMetadata.fileType == FileType.Document) {
                    if (cardView.linear_file_icon != null) {
                        cardView.linear_file_icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                    } else {
                        cardView.grid_file_icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                    }
                } else {
                    if (cardView.linear_file_icon != null) {
                        cardView.linear_file_icon.setImageResource(R.drawable.round_folder_white_18dp)
                    } else {
                        cardView.grid_file_icon.setImageResource(R.drawable.round_folder_white_18dp)
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
