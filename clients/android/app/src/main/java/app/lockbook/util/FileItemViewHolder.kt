package app.lockbook.util

import android.text.SpannableString
import android.view.View
import android.widget.ImageView
import android.widget.TextView
import androidx.constraintlayout.widget.ConstraintLayout
import app.lockbook.R
import com.afollestad.recyclical.ViewHolder
import com.google.android.material.button.MaterialButton
import net.lockbook.File
import net.lockbook.File.FileType

sealed class FileViewHolderInfo(open val fileMetadata: File, open val needToBePushed: Boolean, open val needsToBePulled: Boolean) {
    data class DocumentViewHolderInfo(override val fileMetadata: File, override val needToBePushed: Boolean, override val needsToBePulled: Boolean) : FileViewHolderInfo(fileMetadata, needToBePushed, needsToBePulled)
    data class FolderViewHolderInfo(override val fileMetadata: File, override val needToBePushed: Boolean, override val needsToBePulled: Boolean) : FileViewHolderInfo(fileMetadata, needToBePushed, needsToBePulled)
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

class BasicFileItemHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.linear_move_file_name)
    val icon: ImageView = itemView.findViewById(R.id.linear_move_file_icon)
}

fun List<File>.intoViewHolderInfo(localChanges: HashSet<String>, serverChanges: HashSet<String>?): List<FileViewHolderInfo> = this.map { fileMetadata ->
    when (fileMetadata.type) {
        FileType.Document -> FileViewHolderInfo.DocumentViewHolderInfo(fileMetadata, localChanges.contains(fileMetadata.id), serverChanges?.contains(fileMetadata.id) ?: false)
        FileType.Folder, FileType.Link -> FileViewHolderInfo.FolderViewHolderInfo(fileMetadata, localChanges.contains(fileMetadata.id), serverChanges?.contains(fileMetadata.id) ?: false)
    }
}

class HorizontalTabItemHolder(itemView: View) : ViewHolder(itemView) {
    val name: MaterialButton = itemView.findViewById(R.id.tab_name)
}
class VerticalTabItemHolder(itemView: View) : ViewHolder(itemView) {
    val name: MaterialButton = itemView.findViewById(R.id.tab_name_v)
    val closeButton: MaterialButton = itemView.findViewById(R.id.close_tab)
}

data class SuggestedDocsViewHolderInfo(val fileMetadata: File, val folderName: String)

class SuggestedDocsItemViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.suggested_doc_name)
    val icon: ImageView = itemView.findViewById(R.id.suggested_doc_icon)
    val folderName: TextView = itemView.findViewById(R.id.suggested_docs_parent_folder)
    val lastEdited: TextView = itemView.findViewById(R.id.suggested_doc_last_edited)
}

fun List<File>.intoSuggestedViewHolderInfo(idsAndFiles: Map<String, File>): List<SuggestedDocsViewHolderInfo> = this.map { fileMetadata ->
    SuggestedDocsViewHolderInfo(
        fileMetadata,
        idsAndFiles[fileMetadata.parent]!!.name
    )
}

fun List<FileViewHolderInfo>.intoFileMetadata(): List<File> = this.map { viewHolderInfo -> viewHolderInfo.fileMetadata }

sealed class SearchedDocumentViewHolderInfo(open val id: String, open val path: SpannableString, open val name: SpannableString, open val score: Int) {
    data class DocumentNameViewHolderInfo(override val id: String, override val path: SpannableString, override val name: SpannableString, override val score: Int) : SearchedDocumentViewHolderInfo(id, path, name, score)
    data class DocumentContentViewHolderInfo(override val id: String, override val path: SpannableString, override val name: SpannableString, override val score: Int, val content: SpannableString) : SearchedDocumentViewHolderInfo(id, path, name, score)
}

class SearchedDocumentNameViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.searched_document_name)
    val path: TextView = itemView.findViewById(R.id.searched_document_name_path)
}

class SearchedDocumentContentViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.searched_document_content_name)
    val path: TextView = itemView.findViewById(R.id.searched_document_content_path)
    val content: TextView = itemView.findViewById(R.id.searched_document_content)
}

class SharedFileViewHolder(itemView: View) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.shared_file_name)
    val owner: TextView = itemView.findViewById(R.id.shared_file_owner)
    val icon: ImageView = itemView.findViewById(R.id.shared_file_icon)

    val openMenu: MaterialButton = itemView.findViewById(R.id.open_menu)
}

class SeparatorViewHolder(itemView: View) : ViewHolder(itemView) {
    val date: TextView = itemView.findViewById(R.id.separator_date)
}
