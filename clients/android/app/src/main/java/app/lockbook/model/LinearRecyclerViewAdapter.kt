package app.lockbook.model

import android.content.res.Resources
import android.view.LayoutInflater
import android.view.ViewGroup
import android.widget.TextView
import androidx.cardview.widget.CardView
import androidx.core.content.res.ResourcesCompat
import app.lockbook.App
import app.lockbook.R
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.linear_layout_file_item.view.*
import timber.log.Timber
import java.sql.Date
import java.sql.Timestamp

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

    private fun setReadableLastSynced(description: TextView, resources: Resources, metadataVersion: Long) {
        when (val calculateUsageResult = CoreModel.calculateLastSynced(config)) {
            is Ok -> description.text = resources.getString(
                R.string.last_synced,
                calculateUsageResult.value
            )
            is Err -> when (val error = calculateUsageResult.error) {
                is GetLastSynced.Unexpected -> {
                    description.text = resources.getString(
                        R.string.last_synced,
                        if (metadataVersion != 0L) Date(Timestamp(metadataVersion).time) else resources.getString(R.string.never_synced)
                    )
                    Timber.e("Unable to calculate last synced: ${error.error}")
                }
            }
        }.exhaustive
    }

    override fun onBindViewHolder(holder: FileViewHolder, position: Int) {
        val item = files[position]

        holder.fileMetadata = item
        holder.cardView.linear_file_name.text = item.name.removeSuffix(".draw")
        setReadableLastSynced(holder.cardView.linear_file_description, holder.cardView.resources, item.metadataVersion)

        when {
            selectedFiles[position] -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.selectedFileBackground,
                        App.instance.theme
                    )
                )
                holder.cardView.linear_file_icon.setImageResource(R.drawable.ic_baseline_check_24)
            }
            item.fileType == FileType.Document && item.name.endsWith(".draw") -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                holder.cardView.linear_file_icon.setImageResource(R.drawable.ic_baseline_border_color_24)
            }
            item.fileType == FileType.Document -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                holder.cardView.linear_file_icon.setImageResource(R.drawable.ic_baseline_insert_drive_file_24)
            }
            else -> {
                holder.cardView.background.setTint(
                    ResourcesCompat.getColor(
                        App.instance.resources,
                        R.color.colorPrimaryDark,
                        App.instance.theme
                    )
                )
                holder.cardView.linear_file_icon.setImageResource(R.drawable.round_folder_white_18dp)
            }
        }
    }
}
