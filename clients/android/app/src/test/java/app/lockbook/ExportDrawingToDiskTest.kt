package app.lockbook

import app.lockbook.core.exportDrawingToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class ExportDrawingToDiskTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lb_external_interface")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, false, createRandomPath()))
    }

    @Test
    fun exportDrawingToDiskOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(document.id, Json.encodeToString(Drawing())).unwrapOk()

        CoreModel.exportDrawingToDisk(
            document.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapOk()
    }

    @Test
    fun exportDrawingToDiskFileDoesNotExist() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.getRoot().unwrapOk()

        CoreModel.exportDrawingToDisk(
            generateId(),
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType(ExportDrawingToDiskError.FileDoesNotExist)
    }

    @Test
    fun exportDrawingToDiskInvalidDrawing() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(document.id, "an invalid drawing").unwrapOk()

        CoreModel.exportDrawingToDisk(
            document.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType(ExportDrawingToDiskError.InvalidDrawing)
    }

    @Test
    fun exportDrawingToDiskFolderTreatedAsDrawing() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.exportDrawingToDisk(
            folder.id,
            SupportedImageFormats.Jpeg,
            generateFakeRandomPath()
        ).unwrapErrorType(ExportDrawingToDiskError.FolderTreatedAsDrawing)
    }

    @Test
    fun exportDrawingToDiskBadPath() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(document.id, Json.encodeToString(Drawing())).unwrapOk()

        CoreModel.exportDrawingToDisk(document.id, SupportedImageFormats.Jpeg, "")
            .unwrapErrorType(ExportDrawingToDiskError.BadPath)
    }

    @Test
    fun exportDrawingToDiskFileAlreadyExistsInDisk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(document.id, Json.encodeToString(Drawing())).unwrapOk()

        val path = generateFakeRandomPath()

        CoreModel.exportDrawingToDisk(document.id, SupportedImageFormats.Jpeg, path)
            .unwrapOk()

        CoreModel.exportDrawingToDisk(document.id, SupportedImageFormats.Jpeg, path)
            .unwrapErrorType(ExportDrawingToDiskError.FileAlreadyExistsInDisk)
    }
}
