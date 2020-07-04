package app.lockbook.listfiles

import android.view.LayoutInflater
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.ClientFileMetadata
import app.lockbook.R

class ListFilesAdapter: RecyclerView.Adapter<ListFilesAdapter.ListFilesViewHolder>() {

    var filesFolders = listOf<ClientFileMetadata>()
        set(value) {
            field = value
            notifyDataSetChanged()
        }

    inner class ListFilesViewHolder(val textView: TextView): RecyclerView.ViewHolder(textView) {
        lateinit var clientFileMetadata: ClientFileMetadata
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ListFilesViewHolder {
        val layoutInflater = LayoutInflater.from(parent.context)
        val view = layoutInflater.inflate(R.layout.recyclerview_content_list_files, parent, false) as TextView

        return ListFilesViewHolder(view)
    }

    override fun getItemCount(): Int = filesFolders.size

    override fun onBindViewHolder(holder: ListFilesViewHolder, position: Int) {
        val item = filesFolders[position]

        holder.clientFileMetadata = item
        holder.textView.text = item.name
    }
}