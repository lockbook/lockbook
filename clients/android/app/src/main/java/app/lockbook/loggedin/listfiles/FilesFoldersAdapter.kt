package app.lockbook.loggedin.listfiles

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.cardview.widget.CardView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.FileType
import app.lockbook.R
import kotlinx.android.synthetic.main.recyclerview_content_files_folders.view.*

class FilesFoldersAdapter(val clickInterface: ClickInterface): RecyclerView.Adapter<FilesFoldersAdapter.ListFilesViewHolder>() {

    var filesFolders = listOf<FileMetadata>()
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ListFilesViewHolder {
        val layoutInflater = LayoutInflater.from(parent.context)
        val view = layoutInflater.inflate(R.layout.recyclerview_content_files_folders, parent, false) as CardView

        return ListFilesViewHolder(view)
    }

    override fun getItemCount(): Int = filesFolders.size

    override fun onBindViewHolder(holder: ListFilesViewHolder, position: Int) {
        val item = filesFolders[position]

        holder.fileMetadata = item
        holder.cardView.file_folder_name.text = item.name
        holder.cardView.file_folder_description.text = item.id

        if(item.file_type == FileType.Document) {
            holder.cardView.file_folder_icon.setImageResource(R.drawable.ic_file_24)
        } else {
            holder.cardView.file_folder_icon.setImageResource(R.drawable.ic_folder_24)
        }
    }

    inner class ListFilesViewHolder(val cardView: CardView): RecyclerView.ViewHolder(cardView) {
        lateinit var fileMetadata: FileMetadata

        init {
            cardView.setOnClickListener {
                clickInterface.onItemClick(adapterPosition)
            }

            cardView.setOnLongClickListener{
                clickInterface.onLongClick(adapterPosition)
                true
            }

        }
    }

}