package app.lockbook.loggedin.mainscreen

import app.lockbook.core.getChildren
import app.lockbook.core.getFileMetadata
import app.lockbook.utils.Document
import app.lockbook.utils.EncryptedValueWithNonce
import app.lockbook.utils.FileMetadata
import com.beust.klaxon.Klaxon

class FileFolderModel(private val path: String) {

    private val json = Klaxon()
    lateinit var parentFileMetadata: FileMetadata

    fun setParentToRoot() {
        val root: FileMetadata? = json.parse(app.lockbook.core.getRoot(path))

        if (root != null) {
            parentFileMetadata = root
        }
    }

    fun getChildrenOfParent(): List<FileMetadata> {
        val children: List<FileMetadata>? =
            json.parseArray(getChildren(path, parentFileMetadata.id))

        if (children != null) {
            return children.filter { it.id != it.parent }
        }

        return listOf()
    }

    fun getSiblingsOfParent(): List<FileMetadata> {
        val children: List<FileMetadata>? =
            json.parseArray(getChildren(path, parentFileMetadata.parent))

        children?.let {
            val editedChildren =
                it.filter { fileMetaData -> fileMetaData.id != fileMetaData.parent }
            getParentOfParent()
            return editedChildren
        }

        return listOf()
    }

    fun getFile(fileUuid: String): Document {
        val file: Document? = json.parse(app.lockbook.core.getFile(path, fileUuid))
        if (file != null) {
            return file
        }

        return Document(EncryptedValueWithNonce("", "")) // better way to do this maybe?
    }

    private fun getParentOfParent() {
        val parent: FileMetadata? = json.parse(getFileMetadata(path, parentFileMetadata.parent))

        if (parent != null) {
            parentFileMetadata = parent
        }
    }
}