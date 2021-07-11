package app.lockbook

import app.lockbook.core.exportDrawingToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.writeToDocument(config, document.id, Klaxon().toJsonString(Drawing())).unwrap()

        CoreModel.exportDrawingToDisk(
            config,
            document.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrap()
    }

    @Test
    fun exportDrawingToDiskNoAccount() {
        CoreModel.exportDrawingToDisk(
            config,
            generateId(),
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType<ExportDrawingToDiskError.NoAccount>()
    }

    @Test
    fun exportDrawingToDiskFileDoesNotExist() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.getRoot(config).unwrap()

        CoreModel.exportDrawingToDisk(
            config,
            generateId(),
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType<ExportDrawingToDiskError.FileDoesNotExist>()
    }

    @Test
    fun exportDrawingToDiskInvalidDrawing() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.writeToDocument(config, document.id, "").unwrap()

        CoreModel.exportDrawingToDisk(
            config,
            document.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType<ExportDrawingToDiskError.InvalidDrawing>()
    }

    @Test
    fun exportDrawingToDiskFolderTreatedAsDrawing() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.exportDrawingToDisk(
            config,
            folder.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType<ExportDrawingToDiskError.FolderTreatedAsDrawing>()
    }

    @Test
    fun exportDrawingToDiskBadPath() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.writeToDocument(config, document.id, Klaxon().toJsonString(Drawing())).unwrap()

        CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, "")
            .unwrapErrorType<ExportDrawingToDiskError.BadPath>()
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

        CoreModel.writeToDocument(config, document.id, Klaxon().toJsonString(Drawing())).unwrap()

        val path = generateFakeRandomPath()

        CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, path)
            .unwrap()

        CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, path)
            .unwrapErrorType<ExportDrawingToDiskError.FileAlreadyExistsInDisk>()
    }

    @Test
    fun exportDrawingToDiskUnexpectedError() {
        Klaxon().converter(exportDrawingToDiskConverter)
            .parse<Result<Unit, ExportDrawingToDiskError>>(exportDrawingToDisk("", "", "", ""))
            .unwrapErrorType<ExportDrawingToDiskError.Unexpected>()
    }
}
