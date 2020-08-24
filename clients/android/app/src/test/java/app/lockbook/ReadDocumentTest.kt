package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.FileType
import app.lockbook.utils.ReadDocumentError
import com.beust.klaxon.Klaxon
import org.junit.Before
import org.junit.Test

class ReadDocumentTest {

    private val coreModel = CoreModel(Config(path))

    @Before
    fun loadLib() {
        loadLockbookCore()
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
    }

    @Test
    fun readDocument() {
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        coreModel.getDocumentContent(document.id).component1()!!
    }

    @Test
    fun readFolder() {
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val readDocumentError = coreModel.getDocumentContent(folder.id).component2()!!
        require(readDocumentError is ReadDocumentError.TreatedFolderAsDocument)
    }

    @Test
    fun readDocumentDoesNotExist() {
        val readDocumentError = coreModel.getDocumentContent(generateAlphaString()).component2()!!
        require(readDocumentError is ReadDocumentError.FileDoesNotExist)
    }
}