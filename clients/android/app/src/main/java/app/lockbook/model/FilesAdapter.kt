package app.lockbook.model

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.cardview.widget.CardView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.util.ClickInterface
import app.lockbook.util.FileMetadata
import app.lockbook.util.FileType
import kotlinx.android.synthetic.main.recyclerview_content_files.view.*
import java.sql.Date
import java.sql.Timestamp

class FilesAdapter(val clickInterface: ClickInterface) :
    RecyclerView.Adapter<FilesAdapter.ListFilesViewHolder>() {

    var files = listOf<FileMetadata>()
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ListFilesViewHolder {
        val layoutInflater = LayoutInflater.from(parent.context)
        val view =
            layoutInflater.inflate(R.layout.recyclerview_content_files, parent, false) as CardView

        return ListFilesViewHolder(view)
    }

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
        if (item.fileType == FileType.Document) {
            holder.cardView.file_icon.setImageResource(R.drawable.round_insert_drive_file_white_18dp)
        } else {
            holder.cardView.file_icon.setImageResource(R.drawable.round_folder_white_18dp)
        }
    }

    inner class ListFilesViewHolder(val cardView: CardView) : RecyclerView.ViewHolder(cardView) {
        lateinit var fileMetadata: FileMetadata
        private var selected = false

        init {
            cardView.setOnClickListener {
                clickInterface.onItemClick(adapterPosition)
            }

            cardView.setOnLongClickListener {
                selected = !selected
                if(selected) {
                    cardView.file_icon.setImageResource(R.drawable.ic_baseline_check_24)
                } else {
                    if (fileMetadata.fileType == FileType.Document) {
                        cardView.file_icon.setImageResource(R.drawable.round_insert_drive_file_white_18dp)
                    } else {
                        cardView.file_icon.setImageResource(R.drawable.round_folder_white_18dp)
                    }
                }
                clickInterface.onLongClick(adapterPosition)
                true
            }
        }
    }
}
