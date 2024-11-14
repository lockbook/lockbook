package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.GetFileByIdError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetFileByIdTest {

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
    fun getFileByIdOk() {
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

        CoreModel.getFileById(document.id).unwrapOk()

        CoreModel.getFileById(folder.id).unwrapOk()
    }

    @Test
    fun getFileByIdNoFile() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.getFileById(generateId())
            .unwrapErrorType(GetFileByIdError.NoFileWithThatId)
    }
}
