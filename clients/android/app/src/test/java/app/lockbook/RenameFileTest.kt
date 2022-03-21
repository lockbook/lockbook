package app.lockbook

import app.lockbook.core.readDocument
import app.lockbook.core.renameFile
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.renameFile(config, document.id, generateAlphaString()).unwrap()

        CoreModel.renameFile(config, folder.id, generateAlphaString()).unwrap()
    }

    @Test
    fun renameFileDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        CoreModel.getRoot(config).unwrap()

        CoreModel.renameFile(config, generateId(), generateAlphaString())
            .unwrapErrorType(RenameFileError.FileDoesNotExist)
    }

    @Test
    fun renameFileContainsSlash() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.renameFile(config, document.id, "/")
            .unwrapErrorType(RenameFileError.NewNameContainsSlash)

        CoreModel.renameFile(config, folder.id, "/")
            .unwrapErrorType(RenameFileError.NewNameContainsSlash)
    }

    @Test
    fun renameFileNameNotAvailable() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val fileName = generateAlphaString()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            fileName,
            FileType.Document
        ).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.renameFile(config, folder.id, fileName)
            .unwrapErrorType(RenameFileError.FileNameNotAvailable)
    }

    @Test
    fun renameFileEmpty() {
        val fileName = generateAlphaString()
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            fileName,
            FileType.Folder
        ).unwrap()

        CoreModel.renameFile(config, document.id, "").unwrapErrorType(RenameFileError.NewNameEmpty)

        CoreModel.renameFile(config, folder.id, "").unwrapErrorType(RenameFileError.NewNameEmpty)
    }

    @Test
    fun cannotRenameRoot() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        CoreModel.renameFile(config, rootFileMetadata.id, generateAlphaString())
            .unwrapErrorType(RenameFileError.CannotRenameRoot)
    }

    @Test
    fun renameFileUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<Unit, RenameFileError>>(
            renameFile("", "", "")
        ).unwrapUnexpected()
    }
}
