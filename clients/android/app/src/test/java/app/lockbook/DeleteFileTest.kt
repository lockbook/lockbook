package app.lockbook

import app.lockbook.core.deleteFile
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileDeleteError
import app.lockbook.util.FileType
import app.lockbook.util.IntermCoreResult
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class DeleteFileTest {
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
    fun deleteFileOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.deleteFile(config, document.id).unwrapOk()

        CoreModel.deleteFile(config, folder.id).unwrapOk()
    }

    @Test
    fun deleteFileNoFileWithThatId() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.deleteFile(config, generateId()).unwrapErrorType(FileDeleteError.FileDoesNotExist)
    }

    @Test
    fun deleteFileCannotDeleteRoot() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        CoreModel.deleteFile(config, rootFileMetadata.id)
            .unwrapErrorType(FileDeleteError.CannotDeleteRoot)
    }

    @Test
    fun deleteFileUnexpectedError() {
        CoreModel.deleteFileParser.decodeFromString<IntermCoreResult<Unit, FileDeleteError>>(
            deleteFile("", "")
        ).unwrapUnexpected()
    }
}
