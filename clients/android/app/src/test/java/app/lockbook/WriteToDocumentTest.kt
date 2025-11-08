package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.WriteToDocumentError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class WriteToDocumentTest {

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
    fun writeToDocumentOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.writeToDocument(document.id, "").unwrapOk()
    }

    @Test
    fun writeToDocumentFileDoesNotExist() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.writeToDocument(generateId(), "")
            .unwrapErrorType(WriteToDocumentError.FileDoesNotExist)
    }

    @Test
    fun writeToDocumentFolderTreatedAsDocument() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.writeToDocument(folder.id, "")
            .unwrapErrorType(WriteToDocumentError.FolderTreatedAsDocument)
    }
}
