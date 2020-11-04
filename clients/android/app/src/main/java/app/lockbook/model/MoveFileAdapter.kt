package app.lockbook.model

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.cardview.widget.CardView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.util.FileMetadata
import app.lockbook.util.RegularClickInterface
import kotlinx.android.synthetic.main.recyclerview_content_files.view.*
import java.sql.Date
import java.sql.Timestamp

class MoveFileAdapter(val clickInterface: RegularClickInterface) :
    RecyclerView.Adapter<MoveFileAdapter.MoveFileViewHolder>() {

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

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): MoveFileViewHolder =
        MoveFileViewHolder(
            LayoutInflater.from(parent.context)
                .inflate(R.layout.recyclerview_content_files, parent, false) as CardView
        )

    override fun getItemCount(): Int = files.size

    override fun onBindViewHolder(holder: MoveFileViewHolder, position: Int) {
        val item = files[position]

        val date = Date(Timestamp(item.metadataVersion).time)
        holder.fileMetadata = item
        holder.cardView.file_name.text = item.name
        holder.cardView.file_description.text = holder.cardView.resources.getString(
            R.string.last_synced,
            if (item.metadataVersion != 0L) date else holder.cardView.resources.getString(R.string.never_synced)
        )

        holder.cardView.file_icon.setImageResource(R.drawable.round_folder_white_18dp)
    }

    inner class MoveFileViewHolder(val cardView: CardView) : RecyclerView.ViewHolder(cardView) {
        lateinit var fileMetadata: FileMetadata

        init {
            cardView.setOnClickListener {
                clickInterface.onItemClick(adapterPosition)
            }
        }
    }
}
