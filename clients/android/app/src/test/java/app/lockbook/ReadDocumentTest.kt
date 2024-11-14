package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.ReadDocumentError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class ReadDocumentTest {

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
    fun readDocumentOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.readDocument(document.id).unwrapOk()
    }

    @Test
    fun readFolder() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.readDocument(folder.id)
            .unwrapErrorType(ReadDocumentError.TreatedFolderAsDocument)
    }

    @Test
    fun readDocumentDoesNotExist() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.readDocument(generateId())
            .unwrapErrorType(ReadDocumentError.FileDoesNotExist)
    }
}
