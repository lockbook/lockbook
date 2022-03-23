package app.lockbook

import app.lockbook.core.readDocument
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.ReadDocumentError
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ReadDocumentTest {
    var config = Config(createRandomPath())

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        config = Config(createRandomPath())
    }

    @Test
    fun readDocumentOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.readDocument(config, document.id).unwrapOk()
    }

    @Test
    fun readFolder() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.readDocument(config, folder.id)
            .unwrapErrorType(ReadDocumentError.TreatedFolderAsDocument)
    }

    @Test
    fun readDocumentDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.readDocument(config, generateId())
            .unwrapErrorType(ReadDocumentError.FileDoesNotExist)
    }

    @Test
    fun readDocumentUnexpectedError() {
        CoreModel.readDocumentParser.decodeFromString<IntermCoreResult<String, ReadDocumentError>>(
            readDocument("", "")
        ).unwrapUnexpected()
    }
}
