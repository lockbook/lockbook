package app.lockbook

import app.lockbook.core.saveDocumentToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.saveDocumentToDisk(config, document.id, generateFakeRandomPath()).unwrap()
    }

    @Test
    fun saveDocumentToDiskFolder() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.saveDocumentToDisk(config, folder.id, generateFakeRandomPath())
            .unwrapErrorType<SaveDocumentToDiskError.TreatedFolderAsDocument>()
    }

    @Test
    fun saveDocumentToDiskDoesNotExist() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.saveDocumentToDisk(config, generateId(), generateFakeRandomPath())
            .unwrapErrorType<SaveDocumentToDiskError.FileDoesNotExist>()
    }

    @Test
    fun saveDocumentToDiskBadPath() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.saveDocumentToDisk(config, document.id, "")
            .unwrapErrorType<SaveDocumentToDiskError.BadPath>()
    }

    @Test
    fun exportDrawingToDiskFileAlreadyExistsInDisk() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        val path = generateFakeRandomPath()

        CoreModel.saveDocumentToDisk(config, document.id, path).unwrap()

        CoreModel.saveDocumentToDisk(config, document.id, path)
            .unwrapErrorType<SaveDocumentToDiskError.FileAlreadyExistsInDisk>()
    }

    @Test
    fun saveDocumentToDiskUnexpectedError() {
        Klaxon().converter(saveDocumentToDiskConverter)
            .parse<Result<Unit, SaveDocumentToDiskError>>(
                saveDocumentToDisk("", "", "")
            ).unwrapErrorType<SaveDocumentToDiskError.Unexpected>()
    }
}
