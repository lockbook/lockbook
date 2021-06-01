package app.lockbook.model

import android.view.LayoutInflater
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.TextView
import androidx.cardview.widget.CardView
import androidx.core.content.res.ResourcesCompat
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.*

class LinearRecyclerViewAdapter(listFilesClickInterface: ListFilesClickInterface, filesDir: String) :
    GeneralViewAdapter(listFilesClickInterface) {

    val config = Config(filesDir)

    override var files = listOf<FileMetadata>()
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    override var selectedFiles = MutableList(files.size) { false }
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): FileViewHolder =
        FileViewHolder(
            LayoutInflater.from(parent.context)
                .inflate(R.layout.linear_layout_file_item, parent, false) as CardView
        )

    override fun getItemCount(): Int = files.size

    override fun onBindViewHolder(holder: FileViewHolder, position: Int) {
        val item = files[position]

        holder.fileMetadata = item
        holder.cardView.findViewById<TextView>(R.id.linear_file_name).text = item.name
        holder.cardView.findViewById<TextView>(R.id.linear_file_description).text = holder.cardView.resources.getString(
            R.string.last_synced,
            CoreModel.convertToHumanDuration(item.metadataVersion)
        )

        val fileIcon = holder.cardView.findViewById<ImageView>(R.id.linear_file_icon)
        when {
            selectedFiles[position] -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.selectedFileBackground,
                        App.instance.theme
                    )
                )
                fileIcon.setImageResource(R.drawable.ic_baseline_check_24)
            }
            item.fileType == FileType.Document && item.name.endsWith(".draw") -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                fileIcon.setImageResource(R.drawable.ic_baseline_border_color_24)
            }
            item.fileType == FileType.Document -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                fileIcon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
            }
            else -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                fileIcon.setImageResource(R.drawable.round_folder_white_18dp)
            }
        }
    }
}
