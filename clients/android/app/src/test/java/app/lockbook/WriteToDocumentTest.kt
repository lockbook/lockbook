package app.lockbook

import app.lockbook.core.backgroundSync
import app.lockbook.core.writeDocument
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.writeToDocument(config, document.id, "").unwrap()
    }

    @Test
    fun writeToDocumentFileDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        CoreModel.writeToDocument(config, generateId(), "")
            .unwrapErrorType(WriteToDocumentError.FileDoesNotExist)
    }

    @Test
    fun writeToDocumentFolderTreatedAsDocument() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.writeToDocument(config, folder.id, "")
            .unwrapErrorType(WriteToDocumentError.FolderTreatedAsDocument)
    }

    @Test
    fun writeToDocumentUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<Unit, WriteToDocumentError>>(
            writeDocument("", "", "")
        ).unwrapUnexpected()
    }
}
