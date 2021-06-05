package app.lockbook

import app.lockbook.core.exportDrawing
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.writeContentToDocument(config, document.id, Klaxon().toJsonString(Drawing())).component1()
        )

        assertType<List<Byte>>(
            CoreModel.exportDrawing(config, document.id, SupportedImageFormats.Jpeg).component1()
        )
    }

    @Test
    fun exportDrawingNoAccount() {
        assertType<ExportDrawingError.NoAccount>(
            CoreModel.exportDrawing(config, generateId(), SupportedImageFormats.Jpeg).component2()
        )
    }

    @Test
    fun exportDrawingFileDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<ExportDrawingError.FileDoesNotExist>(
            CoreModel.exportDrawing(config, generateId(), SupportedImageFormats.Jpeg).component2()
        )
    }

    @Test
    fun exportDrawingInvalidDrawing() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.writeContentToDocument(config, document.id, "").component1()
        )

        assertType<ExportDrawingError.InvalidDrawing>(
            CoreModel.exportDrawing(config, document.id, SupportedImageFormats.Jpeg).component2()
        )
    }

    @Test
    fun exportDrawingFolderTreatedAsDrawing() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<ExportDrawingError.FolderTreatedAsDrawing>(
            CoreModel.exportDrawing(config, folder.id, SupportedImageFormats.Jpeg).component2()
        )
    }

    @Test
    fun createFileUnexpectedError() {
        assertType<ExportDrawingError.Unexpected>(
            Klaxon().converter(exportDrawingConverter)
                .parse<Result<List<Byte>, ExportDrawingError>>(exportDrawing("", "", ""))?.component2()
        )
    }
}
