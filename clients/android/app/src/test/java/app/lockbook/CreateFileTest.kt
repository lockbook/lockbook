package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.CreateFileError
import app.lockbook.util.FileType
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class CreateFileTest {

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
    fun createFileOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()
    }

    @Test
    fun createFileContainsSlash() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        CoreModel.createFile(
            rootFileMetadata.id,
            "/",
            FileType.Document
        ).unwrapErrorType(CreateFileError.FileNameContainsSlash)

        CoreModel.createFile(
            rootFileMetadata.id,
            "/",
            FileType.Folder
        ).unwrapErrorType(CreateFileError.FileNameContainsSlash)
    }

    @Test
    fun createFileEmpty() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        CoreModel.createFile(
            rootFileMetadata.id,
            "",
            FileType.Document
        ).unwrapErrorType(CreateFileError.FileNameEmpty)

        CoreModel.createFile(
            rootFileMetadata.id,
            "",
            FileType.Folder
        ).unwrapErrorType(CreateFileError.FileNameEmpty)
    }

    @Test
    fun createFileNotAvailable() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()
        val fileName = generateAlphaString()

        CoreModel.createFile(
            rootFileMetadata.id,
            fileName,
            FileType.Document
        ).unwrapOk()

        CoreModel.createFile(
            rootFileMetadata.id,
            fileName,
            FileType.Folder
        ).unwrapErrorType(CreateFileError.FileNameNotAvailable)
    }
}
