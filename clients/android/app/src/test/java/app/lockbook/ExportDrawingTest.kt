package app.lockbook

import app.lockbook.core.exportDrawing
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ExportDrawingTest {
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
    fun exportDrawingOk() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.writeToDocument(config, document.id, Klaxon().toJsonString(Drawing()))
            .unwrap()

        CoreModel.exportDrawing(config, document.id, SupportedImageFormats.Jpeg).unwrap()
    }

    @Test
    fun exportDrawingNoAccount() {
        CoreModel.exportDrawing(config, generateId(), SupportedImageFormats.Jpeg)
            .unwrapErrorType<ExportDrawingError.NoAccount>()
    }

    @Test
    fun exportDrawingFileDoesNotExist() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.getRoot(config).unwrap()

        CoreModel.exportDrawing(config, generateId(), SupportedImageFormats.Jpeg)
            .unwrapErrorType<ExportDrawingError.FileDoesNotExist>()
    }

    @Test
    fun exportDrawingInvalidDrawing() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.writeToDocument(config, document.id, "").unwrap()

        CoreModel.exportDrawing(config, document.id, SupportedImageFormats.Jpeg)
            .unwrapErrorType<ExportDrawingError.InvalidDrawing>()
    }

    @Test
    fun exportDrawingFolderTreatedAsDrawing() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.exportDrawing(config, folder.id, SupportedImageFormats.Jpeg)
            .unwrapErrorType<ExportDrawingError.FolderTreatedAsDrawing>()
    }

    @Test
    fun unexpectedDrawingUnexpectedError() {
        Klaxon().converter(exportDrawingConverter)
            .parse<Result<List<Byte>, ExportDrawingError>>(exportDrawing("", "", ""))
            .unwrapErrorType<ExportDrawingError.Unexpected>()
    }
}
