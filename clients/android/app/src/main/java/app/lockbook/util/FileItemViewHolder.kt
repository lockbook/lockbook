package app.lockbook.util

import android.content.res.ColorStateList
import android.text.SpannableString
import android.view.View
import android.widget.ImageView
import android.widget.TextView
import androidx.annotation.AttrRes
import androidx.annotation.ColorRes
import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import androidx.core.content.ContextCompat
import app.lockbook.R
import com.afollestad.recyclical.ViewHolder
import com.google.android.material.button.MaterialButton
import com.google.android.material.color.MaterialColors
import com.google.android.material.listitem.ListItemCardView
import net.lockbook.File
import net.lockbook.File.FileType
import java.util.UUID
import com.google.android.material.R as MaterialR

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

class FileMetadataViewHolder(
    itemView: View,
) : com.google.android.material.listitem.ListItemViewHolder(itemView) {
    val fileItemHolder: ListItemCardView = itemView.findViewById(R.id.file_metadata_item_holder)
    val title: TextView = itemView.findViewById(R.id.file_metadata_title)
    val subtitle: TextView = itemView.findViewById(R.id.file_metadata_subtitle)
    val icon: ImageView = itemView.findViewById(R.id.file_metadata_icon)
    val statusIcon: ImageView = itemView.findViewById(R.id.file_metadata_badge)
    val trailingButton: MaterialButton = itemView.findViewById(R.id.file_metadata_trailing_button)

    fun bind(row: FileMetadataRowInfo) {
        fileItemHolder.isSelected = row.isSelected
        fileItemHolder.isChecked = row.isChecked
        fileItemHolder.setOnClickListener(null)
        fileItemHolder.setOnLongClickListener(null)
        fileItemHolder.setCardBackgroundColor(row.background.resolve(itemView))

        icon.setImageResource(row.iconRes)
        title.text = row.title

        subtitle.text = row.subtitle
        subtitle.visibility = if (row.subtitle == null) View.GONE else View.VISIBLE

        statusIcon.setImageDrawable(null)
        statusIcon.visibility = View.GONE
        row.statusIcon?.let {
            statusIcon.setImageResource(it.iconRes)
            statusIcon.visibility = View.VISIBLE
        }

        trailingButton.setOnClickListener(null)
        trailingButton.visibility = View.GONE
        row.trailingButton?.let {
            trailingButton.setIconResource(it.iconRes)
            trailingButton.contentDescription = itemView.context.getString(it.contentDescriptionRes)
            trailingButton.visibility = View.VISIBLE
            trailingButton.setOnClickListener { view -> it.onClick(view) }
        }
    }
}

data class FileMetadataRowInfo(
    val file: File,
    val title: CharSequence,
    val subtitle: CharSequence? = null,
    @param:DrawableRes val iconRes: Int = file.getIconResource(),
    val background: FileMetadataRowBackground = FileMetadataRowBackground.FileTree,
    val statusIcon: FileMetadataStatusIcon? = null,
    val trailingButton: FileMetadataTrailingButton? = null,
    val isSelected: Boolean = false,
    val isChecked: Boolean = false,
)

enum class FileMetadataRowBackground(
    @param:ColorRes private val colorRes: Int? = null,
    @param:AttrRes private val colorAttr: Int? = null,
) {
    FileTree(colorRes = R.color.file_tree_list_item_background),
    SurfaceContainerHigh(colorAttr = MaterialR.attr.colorSurfaceContainerHigh),
    ;

    fun resolve(view: View): ColorStateList =
        colorRes?.let { ContextCompat.getColorStateList(view.context, it) }
            ?: ColorStateList.valueOf(MaterialColors.getColor(view, colorAttr!!))
}

data class FileMetadataStatusIcon(
    @param:DrawableRes val iconRes: Int,
)

data class FileMetadataTrailingButton(
    @param:DrawableRes val iconRes: Int,
    @param:StringRes val contentDescriptionRes: Int,
    val onClick: (View) -> Unit,
)

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
        val file: File,
        val path: SpannableString,
        val name: SpannableString,
    ) : SearchedDocumentViewHolderInfo()

    data class DocumentContentViewHolderInfo(
        val file: File,
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

class SearchedDocumentContentViewHolder(
    itemView: View,
) : ViewHolder(itemView) {
    val itemHolder: ListItemCardView = itemView.findViewById(R.id.searched_document_content_holder)
    val icon: ImageView = itemView.findViewById(R.id.searched_document_content_icon)
    val name: TextView = itemView.findViewById(R.id.searched_document_content_name)
    val path: TextView = itemView.findViewById(R.id.searched_document_content_path)
    val content: TextView = itemView.findViewById(R.id.searched_document_content)
    val showMore: MaterialButton = itemView.findViewById(R.id.searched_document_content_show_more)
}

class SeparatorViewHolder(
    itemView: View,
) : com.afollestad.recyclical.ViewHolder(itemView) {
    val date: TextView = itemView.findViewById(R.id.separator_date)
}
