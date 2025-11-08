package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.FileDeleteError
import app.lockbook.util.FileType
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class DeleteFileTest {

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
    fun deleteFileOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.deleteFile(document.id).unwrapOk()

        CoreModel.deleteFile(folder.id).unwrapOk()
    }

    @Test
    fun deleteFileNoFileWithThatId() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.deleteFile(generateId()).unwrapErrorType(FileDeleteError.FileDoesNotExist)
    }

    @Test
    fun deleteFileCannotDeleteRoot() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        CoreModel.deleteFile(rootFileMetadata.id)
            .unwrapErrorType(FileDeleteError.CannotDeleteRoot)
    }
}
