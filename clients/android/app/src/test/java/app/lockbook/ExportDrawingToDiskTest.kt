package app.lockbook

import app.lockbook.core.exportDrawingToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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

        assertType<Unit>(
            CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, generateFakeRandomPath()).component1()
        )
    }

    @Test
    fun exportDrawingToDiskNoAccount() {
        assertType<ExportDrawingToDiskError.NoAccount>(
            CoreModel.exportDrawingToDisk(config, generateId(), SupportedImageFormats.Jpeg, generateFakeRandomPath()).component2()
        )
    }

    @Test
    fun exportDrawingToDiskFileDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<ExportDrawingToDiskError.FileDoesNotExist>(
            CoreModel.exportDrawingToDisk(config, generateId(), SupportedImageFormats.Jpeg, generateFakeRandomPath()).component2()
        )
    }

    @Test
    fun exportDrawingToDiskInvalidDrawing() {
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

        assertType<ExportDrawingToDiskError.InvalidDrawing>(
            CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, generateFakeRandomPath()).component2()
        )
    }

    @Test
    fun exportDrawingToDiskFolderTreatedAsDrawing() {
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

        assertType<ExportDrawingToDiskError.FolderTreatedAsDrawing>(
            CoreModel.exportDrawingToDisk(config, folder.id, SupportedImageFormats.Jpeg, generateFakeRandomPath()).component2()
        )
    }

    @Test
    fun exportDrawingToDiskBadPath() {
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

        assertType<ExportDrawingToDiskError.BadPath>(
            CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, "").component2()
        )
    }

    @Test
    fun exportDrawingToDiskFileAlreadyExistsInDisk() {
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

        val path = generateFakeRandomPath()

        assertType<Unit>(
            CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, path).component1()
        )

        assertType<ExportDrawingToDiskError.FileAlreadyExistsInDisk>(
            CoreModel.exportDrawingToDisk(config, document.id, SupportedImageFormats.Jpeg, path).component2()
        )
    }

    @Test
    fun exportDrawingToDiskUnexpectedError() {
        assertType<ExportDrawingToDiskError.Unexpected>(
            Klaxon().converter(exportDrawingToDiskConverter)
                .parse<Result<Unit, ExportDrawingToDiskError>>(exportDrawingToDisk("", "", "", ""))?.component2()
        )
    }
}
