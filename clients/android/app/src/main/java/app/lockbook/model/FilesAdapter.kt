package app.lockbook.model

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.cardview.widget.CardView
import androidx.core.content.res.ResourcesCompat
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.FileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.ListFilesClickInterface
import kotlinx.android.synthetic.main.recyclerview_content_files.view.*
import java.sql.Date
import java.sql.Timestamp

class FilesAdapter(val listFilesClickInterface: ListFilesClickInterface) :
    RecyclerView.Adapter<FilesAdapter.ListFilesViewHolder>() {

    var files = listOf<FileMetadata>()
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    var selectedFiles = MutableList(files.size) { false }
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ListFilesViewHolder =
        ListFilesViewHolder(
            LayoutInflater.from(parent.context)
                .inflate(R.layout.recyclerview_content_files, parent, false) as CardView
        )

    override fun getItemCount(): Int = files.size

    override fun onBindViewHolder(holder: ListFilesViewHolder, position: Int) {
        val item = files[position]

        val date = Date(Timestamp(item.metadataVersion).time)
        holder.fileMetadata = item
        holder.cardView.file_name.text = item.name
        holder.cardView.file_description.text = holder.cardView.resources.getString(
            R.string.last_synced,
            if (item.metadataVersion != 0L) date else holder.cardView.resources.getString(R.string.never_synced)
        )
        when {
            selectedFiles[position] -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.selectedFileBackground,
                        App.instance.theme
                    )
                )
                holder.cardView.file_icon.setImageResource(R.drawable.ic_baseline_check_24)
            }
            item.fileType == FileType.Document -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                holder.cardView.file_icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
            }
            else -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                holder.cardView.file_icon.setImageResource(R.drawable.round_folder_white_18dp)
            }
        }
    }

    inner class ListFilesViewHolder(val cardView: CardView) : RecyclerView.ViewHolder(cardView) {
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
                cardView.file_icon.setImageResource(R.drawable.ic_baseline_check_24)
            } else {
                if (fileMetadata.fileType == FileType.Document) {
                    cardView.file_icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
                } else {
                    cardView.file_icon.setImageResource(R.drawable.round_folder_white_18dp)
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
