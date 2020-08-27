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

    private val coreModel = CoreModel(Config(path))

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
            Runtime.getRuntime().exec("rm -rf $path")
        }
    }

    @Before
    fun createDirectory() {
        Runtime.getRuntime().exec("mkdir $path")
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path")
    }

    @Test
    fun writeToDocumentOk() {
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
