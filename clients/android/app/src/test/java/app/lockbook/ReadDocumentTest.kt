package app.lockbook

import app.lockbook.core.readDocument
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ReadDocumentTest {
    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun readDocumentOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        coreModel.getDocumentContent(document.id).component1()!!
    }

    @Test
    fun readFolder() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val readDocumentError = coreModel.getDocumentContent(folder.id).component2()!!
        require(readDocumentError is ReadDocumentError.TreatedFolderAsDocument) {
            "${Klaxon().toJsonString(readDocumentError)} != ${ReadDocumentError.TreatedFolderAsDocument::class.qualifiedName}"
        }
    }

    @Test
    fun readDocumentDoesNotExist() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val readDocumentError = coreModel.getDocumentContent(generateId()).component2()!!
        require(readDocumentError is ReadDocumentError.FileDoesNotExist) {
            "${Klaxon().toJsonString(readDocumentError)} != ${ReadDocumentError.FileDoesNotExist::class.qualifiedName}"
        }
    }

    @Test
    fun readDocumentUnexpectedError() {
        val getDocumentResult: Result<DecryptedValue, ReadDocumentError>? =
            Klaxon().converter(readDocumentConverter).parse(readDocument("", ""))
        val getDocumentError = getDocumentResult!!.component2()!!
        require(getDocumentError is ReadDocumentError.UnexpectedError) {
            "${Klaxon().toJsonString(getDocumentError)} != ${ReadDocumentError.UnexpectedError::class.qualifiedName}"
        }
    }
}
