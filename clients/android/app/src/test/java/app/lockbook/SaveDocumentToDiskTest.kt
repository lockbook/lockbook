package app.lockbook

import app.lockbook.core.saveDocumentToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.SaveDocumentToDiskError
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class SaveDocumentToDiskTest {
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
    fun saveDocumentToDiskOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.saveDocumentToDisk(config, document.id, generateFakeRandomPath()).unwrapOk()
    }

    @Test
    fun saveDocumentToDiskFolder() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.saveDocumentToDisk(config, folder.id, generateFakeRandomPath())
            .unwrapErrorType(SaveDocumentToDiskError.TreatedFolderAsDocument)
    }

    @Test
    fun saveDocumentToDiskDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.saveDocumentToDisk(config, generateId(), generateFakeRandomPath())
            .unwrapErrorType(SaveDocumentToDiskError.FileDoesNotExist)
    }

    @Test
    fun saveDocumentToDiskBadPath() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.saveDocumentToDisk(config, document.id, "")
            .unwrapErrorType(SaveDocumentToDiskError.BadPath)
    }

    @Test
    fun exportDrawingToDiskFileAlreadyExistsInDisk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        val path = generateFakeRandomPath()

        CoreModel.saveDocumentToDisk(config, document.id, path).unwrapOk()

        CoreModel.saveDocumentToDisk(config, document.id, path)
            .unwrapErrorType(SaveDocumentToDiskError.FileAlreadyExistsInDisk)
    }

    @Test
    fun saveDocumentToDiskUnexpectedError() {
        CoreModel.saveDocumentToDiskParser.decodeFromString<IntermCoreResult<Unit, SaveDocumentToDiskError>>(
            saveDocumentToDisk("", "", "")
        ).unwrapUnexpected()
    }
}
