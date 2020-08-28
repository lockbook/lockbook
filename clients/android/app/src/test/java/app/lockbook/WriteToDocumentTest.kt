package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.core.writeDocument
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class WriteToDocumentTest {

    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun writeToDocumentOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        CoreModel.writeContentToDocument(Config(path), document.id, "").component1()!!
    }

    @Test
    fun writeToDocumentFileDoesNotExist() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val writeToDocumentError = CoreModel.writeContentToDocument(Config(path), generateId(), "").component2()!!
        require(writeToDocumentError is WriteToDocumentError.FileDoesNotExist)
    }

    @Test
    fun writeToDocumentFolderTreatedAsDocument() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val writeToDocumentError = CoreModel.writeContentToDocument(Config(path), folder.id, "").component2()!!
        require(writeToDocumentError is WriteToDocumentError.FolderTreatedAsDocument)
    }

    @Test
    fun writeToDocumentUnexpectedError() {
        val writeResult: Result<Unit, WriteToDocumentError>? =
            Klaxon().converter(writeDocumentConverter).parse(writeDocument("", "", ""))
        val writeError = writeResult!!.component2()!!
        require(writeError is WriteToDocumentError.UnexpectedError)
    }
}
