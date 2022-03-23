package app.lockbook

import app.lockbook.core.exportDrawingToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ExportDrawingToDiskTest {
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
    fun exportDrawingToDiskOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(config, document.id, Json.encodeToString(Drawing())).unwrapOk()

        CoreModel.exportDrawingToDisk(
            config,
            document.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapOk()
    }

    @Test
    fun exportDrawingToDiskNoAccount() {
        CoreModel.exportDrawingToDisk(
            config,
            generateId(),
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType(ExportDrawingToDiskError.NoAccount)
    }

    @Test
    fun exportDrawingToDiskFileDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getRoot(config).unwrapOk()

        CoreModel.exportDrawingToDisk(
            config,
            generateId(),
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType(ExportDrawingToDiskError.FileDoesNotExist)
    }

    @Test
    fun exportDrawingToDiskInvalidDrawing() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(config, document.id, "an invalid drawing").unwrapOk()

        CoreModel.exportDrawingToDisk(
            config,
            document.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType(ExportDrawingToDiskError.InvalidDrawing)
    }

    @Test
    fun exportDrawingToDiskFolderTreatedAsDrawing() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.exportDrawingToDisk(
            config,
            folder.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType(ExportDrawingToDiskError.FolderTreatedAsDrawing)
    }

    @Test
    fun exportDrawingToDiskBadPath() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(config, document.id, Json.encodeToString(Drawing())).unwrapOk()

        CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, "")
            .unwrapErrorType(ExportDrawingToDiskError.BadPath)
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

        CoreModel.writeToDocument(config, document.id, Json.encodeToString(Drawing())).unwrapOk()

        val path = generateFakeRandomPath()

        CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, path)
            .unwrapOk()

        CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, path)
            .unwrapErrorType(ExportDrawingToDiskError.FileAlreadyExistsInDisk)
    }

    @Test
    fun exportDrawingToDiskUnexpectedError() {
        CoreModel.exportDrawingToDiskParser.decodeFromString<IntermCoreResult<Unit, ExportDrawingToDiskError>>(
            exportDrawingToDisk("", "", "", "")
        ).unwrapUnexpected()
    }
}
