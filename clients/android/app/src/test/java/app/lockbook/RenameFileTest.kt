package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.RenameFileError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class RenameFileTest {

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
    fun renameFileOk() {
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

        CoreModel.renameFile(document.id, generateAlphaString()).unwrapOk()

        CoreModel.renameFile(folder.id, generateAlphaString()).unwrapOk()
    }

    @Test
    fun renameFileDoesNotExist() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.getRoot().unwrapOk()

        CoreModel.renameFile(generateId(), generateAlphaString())
            .unwrapErrorType(RenameFileError.FileDoesNotExist)
    }

    @Test
    fun renameFileContainsSlash() {
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

        CoreModel.renameFile(document.id, "/")
            .unwrapErrorType(RenameFileError.NewNameContainsSlash)

        CoreModel.renameFile(folder.id, "/")
            .unwrapErrorType(RenameFileError.NewNameContainsSlash)
    }

    @Test
    fun renameFileNameNotAvailable() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val fileName = generateAlphaString()

        CoreModel.createFile(
            rootFileMetadata.id,
            fileName,
            FileType.Document
        ).unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.renameFile(folder.id, fileName)
            .unwrapErrorType(RenameFileError.FileNameNotAvailable)
    }

    @Test
    fun renameFileEmpty() {
        val fileName = generateAlphaString()
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            fileName,
            FileType.Folder
        ).unwrapOk()

        CoreModel.renameFile(document.id, "").unwrapErrorType(RenameFileError.NewNameEmpty)

        CoreModel.renameFile(folder.id, "").unwrapErrorType(RenameFileError.NewNameEmpty)
    }

    @Test
    fun cannotRenameRoot() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        CoreModel.renameFile(rootFileMetadata.id, generateAlphaString())
            .unwrapErrorType(RenameFileError.CannotRenameRoot)
    }
}
