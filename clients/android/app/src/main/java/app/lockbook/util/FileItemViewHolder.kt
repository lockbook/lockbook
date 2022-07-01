package app.lockbook.util

import android.view.View
import android.widget.ImageView
import android.widget.TextView
import androidx.constraintlayout.widget.ConstraintLayout
import app.lockbook.R
import com.afollestad.recyclical.ViewHolder

sealed class FileViewHolderInfo(open val fileMetadata: DecryptedFileMetadata, open val needToBePushed: Boolean, open val needsToBePulled: Boolean) {
    data class DocumentViewHolderInfo(override val fileMetadata: DecryptedFileMetadata, override val needToBePushed: Boolean, override val needsToBePulled: Boolean) : FileViewHolderInfo(fileMetadata, needToBePushed, needsToBePulled)
    data class FolderViewHolderInfo(override val fileMetadata: DecryptedFileMetadata, override val needToBePushed: Boolean, override val needsToBePulled: Boolean) : FileViewHolderInfo(fileMetadata, needToBePushed, needsToBePulled)
}

class DocumentViewHolder(itemView: View) : ViewHolder(itemView) {
    val fileItemHolder: ConstraintLayout = itemView.findViewById(R.id.document_item_holder)
    val name: TextView = itemView.findViewById(R.id.document_name)
    val description: TextView = itemView.findViewById(R.id.document_description)
    val icon: ImageView = itemView.findViewById(R.id.document_icon)
    val actionIcon: ImageView = itemView.findViewById(R.id.document_action_icon)
}

class FolderViewHolder(itemView: View) : ViewHolder(itemView) {
    val fileItemHolder: ConstraintLayout = itemView.findViewById(R.id.folder_item_holder)
    val name: TextView = itemView.findViewById(R.id.folder_name)
    val icon: ImageView = itemView.findViewById(R.id.folder_icon)
    val actionIcon: ImageView = itemView.findViewById(R.id.folder_action_icon)
}

class MoveFileItemViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.linear_move_file_name)
    val icon: ImageView = itemView.findViewById(R.id.linear_move_file_icon)
}

data class RecentFileViewHolderInfo(val fileMetadata: DecryptedFileMetadata, val folderName: String)

class RecentFileItemViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.recent_file_name)
    val icon: ImageView = itemView.findViewById(R.id.recent_file_icon)
    val folderName: TextView = itemView.findViewById(R.id.recent_file_folder)
    val lastEdited: TextView = itemView.findViewById(R.id.recent_file_last_edited)
}

fun List<DecryptedFileMetadata>.intoViewHolderInfo(localChanges: HashSet<String>, serverChanges: HashSet<String>?): List<FileViewHolderInfo> = this.map { fileMetadata ->
    when (fileMetadata.fileType) {
        FileType.Document -> FileViewHolderInfo.DocumentViewHolderInfo(fileMetadata, localChanges.contains(fileMetadata.id), serverChanges?.contains(fileMetadata.id) ?: false)
        FileType.Folder -> FileViewHolderInfo.FolderViewHolderInfo(fileMetadata, localChanges.contains(fileMetadata.id), serverChanges?.contains(fileMetadata.id) ?: false)
    }
}

fun List<DecryptedFileMetadata>.intoRecentViewHolderInfo(files: List<DecryptedFileMetadata>): List<RecentFileViewHolderInfo> = this.map { fileMetadata ->
    RecentFileViewHolderInfo(fileMetadata, files.filter { fileMetadata.parent == it.id }[0].decryptedName)
}

fun List<FileViewHolderInfo>.intoFileMetadata(): List<DecryptedFileMetadata> = this.map { viewHolderInfo -> viewHolderInfo.fileMetadata }
