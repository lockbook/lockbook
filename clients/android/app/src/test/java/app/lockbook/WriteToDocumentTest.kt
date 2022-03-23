package app.lockbook

import app.lockbook.core.writeDocument
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.WriteToDocumentError
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class WriteToDocumentTest {
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
    fun writeToDocumentOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(config, document.id, "").unwrapOk()
    }

    @Test
    fun writeToDocumentFileDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.writeToDocument(config, generateId(), "")
            .unwrapErrorType(WriteToDocumentError.FileDoesNotExist)
    }

    @Test
    fun writeToDocumentFolderTreatedAsDocument() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.writeToDocument(config, folder.id, "")
            .unwrapErrorType(WriteToDocumentError.FolderTreatedAsDocument)
    }

    @Test
    fun writeToDocumentUnexpectedError() {
        CoreModel.writeToDocumentParser.decodeFromString<IntermCoreResult<Unit, WriteToDocumentError>>(
            writeDocument("", "", "")
        ).unwrapUnexpected()
    }
}
