package app.lockbook.util

import android.view.View
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import app.lockbook.R
import com.afollestad.recyclical.ViewHolder

sealed class FileViewHolderInfo(open val fileMetadata: DecryptedFileMetadata) {
    data class DocumentViewHolderInfo(override val fileMetadata: DecryptedFileMetadata): FileViewHolderInfo(fileMetadata)
    data class FolderViewHolderInfo(override val fileMetadata: DecryptedFileMetadata): FileViewHolderInfo(fileMetadata)
}

class DocumentViewHolder(itemView: View) : ViewHolder(itemView) {
    val fileItemHolder: LinearLayout = itemView.findViewById(R.id.document_item_holder)
    val name: TextView = itemView.findViewById(R.id.document_name)
    val description: TextView = itemView.findViewById(R.id.document_description)
    val icon: ImageView = itemView.findViewById(R.id.document_icon)
}

class FolderViewHolder(itemView: View) : ViewHolder(itemView) {
    val fileItemHolder: LinearLayout = itemView.findViewById(R.id.folder_item_holder)
    val name: TextView = itemView.findViewById(R.id.folder_name)
    val icon: ImageView = itemView.findViewById(R.id.folder_icon)
}


class LinearMoveFileItemViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.linear_move_file_name)
    val icon: ImageView = itemView.findViewById(R.id.linear_move_file_icon)
}

fun List<DecryptedFileMetadata>.intoViewHolderInfo(): List<FileViewHolderInfo> = this.map { fileMetadata ->
        when(fileMetadata.fileType) {
            FileType.Document -> FileViewHolderInfo.DocumentViewHolderInfo(fileMetadata)
            FileType.Folder -> FileViewHolderInfo.FolderViewHolderInfo(fileMetadata)
        }
    }

fun List<FileViewHolderInfo>.intoFileMetadata(): List<DecryptedFileMetadata> = this.map { viewHolderInfo -> viewHolderInfo.fileMetadata }

