package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.SaveDocumentToDiskError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class SaveDocumentToDiskTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, false,, createRandomPath()))
    }

    @Test
    fun saveDocumentToDiskOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.saveDocumentToDisk(document.id, generateFakeRandomPath()).unwrapOk()
    }

    @Test
    fun saveDocumentToDiskFolder() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.saveDocumentToDisk(folder.id, generateFakeRandomPath())
            .unwrapErrorType(SaveDocumentToDiskError.TreatedFolderAsDocument)
    }

    @Test
    fun saveDocumentToDiskDoesNotExist() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.saveDocumentToDisk(generateId(), generateFakeRandomPath())
            .unwrapErrorType(SaveDocumentToDiskError.FileDoesNotExist)
    }

    @Test
    fun saveDocumentToDiskBadPath() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.saveDocumentToDisk(document.id, "")
            .unwrapErrorType(SaveDocumentToDiskError.BadPath)
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

        val path = generateFakeRandomPath()

        CoreModel.saveDocumentToDisk(document.id, path).unwrapOk()

        CoreModel.saveDocumentToDisk(document.id, path)
            .unwrapErrorType(SaveDocumentToDiskError.FileAlreadyExistsInDisk)
    }
}
