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
import java.util.UUID

sealed class FileViewHolderInfo(
    open val fileMetadata: File,
    open val needToBePushed: Boolean,
    open val needsToBePulled: Boolean,
    open val isShared: Boolean,
) {
    data class DocumentViewHolderInfo(
        override val fileMetadata: File,
        override val needToBePushed: Boolean,
        override val needsToBePulled: Boolean,
        override val isShared: Boolean,
    ) : FileViewHolderInfo(fileMetadata, needToBePushed, needsToBePulled, isShared)

    data class FolderViewHolderInfo(
        override val fileMetadata: File,
        override val needToBePushed: Boolean,
        override val needsToBePulled: Boolean,
        override val isShared: Boolean,
    ) : FileViewHolderInfo(fileMetadata, needToBePushed, needsToBePulled, isShared)
}

class DocumentViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val fileItemHolder: ConstraintLayout = itemView.findViewById(R.id.document_item_holder)
    val name: TextView = itemView.findViewById(R.id.document_name)
    val description: TextView = itemView.findViewById(R.id.document_description)
    val icon: ImageView = itemView.findViewById(R.id.document_icon)
    val actionIcon: ImageView = itemView.findViewById(R.id.document_action_icon)
}

class FolderViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val fileItemHolder: ConstraintLayout = itemView.findViewById(R.id.folder_item_holder)
    val name: TextView = itemView.findViewById(R.id.folder_name)
    val icon: ImageView = itemView.findViewById(R.id.folder_icon)
    val actionIcon: ImageView = itemView.findViewById(R.id.folder_action_icon)
}

class BasicFileItemHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.linear_move_file_name)
    val icon: ImageView = itemView.findViewById(R.id.linear_move_file_icon)
}

fun List<File>.intoViewHolderInfo(
    localChanges: Set<UUID>?,
    serverChanges: Set<UUID>?,
): List<FileViewHolderInfo> =
    this.map { fileMetadata ->
        val isDirtyLocally = localChanges?.contains(UUID.fromString(fileMetadata.id)) ?: false
        val needsToBePulled = serverChanges?.contains(UUID.fromString(fileMetadata.id)) ?: false
        val isShared = fileMetadata.shares.isNotEmpty()

        when (fileMetadata.type) {
            FileType.Document -> {
                FileViewHolderInfo.DocumentViewHolderInfo(fileMetadata, isDirtyLocally, needsToBePulled, isShared)
            }

            FileType.Folder, FileType.Link -> {
                FileViewHolderInfo.FolderViewHolderInfo(
                    fileMetadata,
                    isDirtyLocally,
                    needsToBePulled,
                    isShared,
                )
            }
        }
    }

class HorizontalTabItemHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val name: MaterialButton = itemView.findViewById(R.id.tab_name)
}

class VerticalTabItemHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val name: MaterialButton = itemView.findViewById(R.id.tab_name_v)
    val closeButton: MaterialButton = itemView.findViewById(R.id.close_tab)
}

fun List<FileViewHolderInfo>.intoFileMetadata(): List<File> = this.map { viewHolderInfo -> viewHolderInfo.fileMetadata }

sealed class SearchedDocumentViewHolderInfo {
    data class SectionHeaderViewHolderInfo(
        val title: String,
        val action: String? = null,
        val isFilenameSearchFocused: Boolean = false,
    ) : SearchedDocumentViewHolderInfo()

    data class EmptyViewHolderInfo(
        val message: String,
    ) : SearchedDocumentViewHolderInfo()

    data class DocumentNameViewHolderInfo(
        val id: String,
        val path: SpannableString,
        val name: SpannableString,
    ) : SearchedDocumentViewHolderInfo()

    data class DocumentContentViewHolderInfo(
        val id: String,
        val path: SpannableString,
        val name: SpannableString,
        val contents: List<SpannableString>,
        val totalMatches: Int,
        val showMore: Boolean,
    ) : SearchedDocumentViewHolderInfo()
}

class SearchSectionHeaderViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val title: TextView = itemView.findViewById(R.id.search_section_title)
    val action: MaterialButton = itemView.findViewById(R.id.search_section_action)
}

class SearchEmptyViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val message: TextView = itemView.findViewById(R.id.search_empty_message)
}

class SearchedDocumentNameViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.searched_document_name)
    val path: TextView = itemView.findViewById(R.id.searched_document_name_path)
}

class SearchedDocumentContentViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.searched_document_content_name)
    val path: TextView = itemView.findViewById(R.id.searched_document_content_path)
    val content1: TextView = itemView.findViewById(R.id.searched_document_content_1)
    val content2: TextView = itemView.findViewById(R.id.searched_document_content_2)
    val content3: TextView = itemView.findViewById(R.id.searched_document_content_3)
    val showMore: MaterialButton = itemView.findViewById(R.id.searched_document_content_show_more)
}

class SharedFileViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val name: TextView = itemView.findViewById(R.id.shared_file_name)
    val owner: TextView = itemView.findViewById(R.id.shared_file_owner)
    val icon: ImageView = itemView.findViewById(R.id.shared_file_icon)

    val openMenu: MaterialButton = itemView.findViewById(R.id.open_menu)
}

class SeparatorViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val date: TextView = itemView.findViewById(R.id.separator_date)
}
