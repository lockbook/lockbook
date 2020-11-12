package app.lockbook.model

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.cardview.widget.CardView
import androidx.core.content.res.ResourcesCompat
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.FileMetadata
import app.lockbook.util.FileType
import app.lockbook.util.ListFilesClickInterface
import kotlinx.android.synthetic.main.grid_layout_file_item.view.*
import java.sql.Date
import java.sql.Timestamp

class GridRecyclerViewAdapter(listFilesClickInterface: ListFilesClickInterface) :
    GeneralViewAdapter(listFilesClickInterface) {

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
                .inflate(R.layout.grid_layout_file_item, parent, false) as CardView
        )

    override fun getItemCount(): Int = files.size

    override fun onBindViewHolder(holder: FileViewHolder, position: Int) {
        val item = files[position]

        val date = Date(Timestamp(item.metadataVersion).time)
        holder.fileMetadata = item
        holder.cardView.grid_file_name.text = item.name
        holder.cardView.grid_file_description1.text = holder.cardView.resources.getString(
            R.string.last_synced,
            if (item.metadataVersion != 0L) date else holder.cardView.resources.getString(R.string.never_synced)
        )
        holder.cardView.grid_file_description2.text = holder.cardView.resources.getString(
            R.string.popup_info_id,
            item.id
        )
        holder.cardView.grid_file_description3.text = holder.cardView.resources.getString(
            R.string.popup_info_content_version,
            item.contentVersion.toString()
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
                holder.cardView.grid_file_icon.setImageResource(R.drawable.ic_baseline_check_24)
            }
            item.fileType == FileType.Document -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                holder.cardView.grid_file_icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
            }
            else -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                holder.cardView.grid_file_icon.setImageResource(R.drawable.round_folder_white_18dp)
            }
        }
    }
}
