package app.lockbook

import app.lockbook.core.renameFile
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.RenameFileError
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class RenameFileTest {
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
    fun renameFileOk() {
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

        CoreModel.renameFile(config, document.id, generateAlphaString()).unwrapOk()

        CoreModel.renameFile(config, folder.id, generateAlphaString()).unwrapOk()
    }

    @Test
    fun renameFileDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getRoot(config).unwrapOk()

        CoreModel.renameFile(config, generateId(), generateAlphaString())
            .unwrapErrorType(RenameFileError.FileDoesNotExist)
    }

    @Test
    fun renameFileContainsSlash() {
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

        CoreModel.renameFile(config, document.id, "/")
            .unwrapErrorType(RenameFileError.NewNameContainsSlash)

        CoreModel.renameFile(config, folder.id, "/")
            .unwrapErrorType(RenameFileError.NewNameContainsSlash)
    }

    @Test
    fun renameFileNameNotAvailable() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val fileName = generateAlphaString()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            fileName,
            FileType.Document
        ).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.renameFile(config, folder.id, fileName)
            .unwrapErrorType(RenameFileError.FileNameNotAvailable)
    }

    @Test
    fun renameFileEmpty() {
        val fileName = generateAlphaString()
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
            fileName,
            FileType.Folder
        ).unwrapOk()

        CoreModel.renameFile(config, document.id, "").unwrapErrorType(RenameFileError.NewNameEmpty)

        CoreModel.renameFile(config, folder.id, "").unwrapErrorType(RenameFileError.NewNameEmpty)
    }

    @Test
    fun cannotRenameRoot() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        CoreModel.renameFile(config, rootFileMetadata.id, generateAlphaString())
            .unwrapErrorType(RenameFileError.CannotRenameRoot)
    }

    @Test
    fun renameFileUnexpectedError() {
        CoreModel.renameFileParser.decodeFromString<IntermCoreResult<Unit, RenameFileError>>(
            renameFile("", "", "")
        ).unwrapUnexpected()
    }
}
