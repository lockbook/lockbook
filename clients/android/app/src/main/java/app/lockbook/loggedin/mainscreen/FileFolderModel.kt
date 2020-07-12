package app.lockbook.loggedin.mainscreen

import android.util.Log
import app.lockbook.core.*
import app.lockbook.utils.DecryptedValue
import app.lockbook.utils.Document
import app.lockbook.utils.EncryptedValueWithNonce
import app.lockbook.utils.FileMetadata
import com.beust.klaxon.Klaxon
import kotlinx.android.synthetic.main.activity_new_file_folder.*

class FileFolderModel(private val path: String) {
    private val json = Klaxon()
    lateinit var parentFileMetadata: FileMetadata
    lateinit var lastDocumentAccessed: FileMetadata

    companion object {
        fun insertFileFolder(path: String, parentUuid: String, fileType: String, name: String) {
            val serializedFileFolder = createFileFolder(path, parentUuid, fileType, name)

            val fileFolder: FileMetadata? = Klaxon().parse(serializedFileFolder)

            fileFolder?.let {
                insertFileFolder(path, serializedFileFolder)
            }
        }
    }

    fun setParentToRoot() {
        val root: FileMetadata? = json.parse(getRoot(path))

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

    private fun getParentOfParent() {
        val parent: FileMetadata? = json.parse(getFileMetadata(path, parentFileMetadata.parent))

        if (parent != null) {
            parentFileMetadata = parent
        }
    }

    fun getDocumentContent(fileUuid: String): String {
        val document: DecryptedValue? = json.parse(readDocument(path, fileUuid))
        if (document != null) {
            return document.secret
        }

        return "" // definitely better way to handle this
    }

    fun writeContentToDocument(content: String) { // have a return type to be handled in case it doesnt work
        writeToDocument(path, lastDocumentAccessed.id, json.toJsonString(DecryptedValue(content)))
    }

}