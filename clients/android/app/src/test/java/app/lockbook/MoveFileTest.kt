package app.lockbook

import app.lockbook.core.migrateDB
import app.lockbook.core.moveFile
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class MoveFileTest {
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
    fun moveFileOk() {
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

        CoreModel.moveFile(config, document.id, folder.id).unwrap()
    }

    @Test
    fun moveFileDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.moveFile(config, generateId(), folder.id)
            .unwrapErrorType<MoveFileError.FileDoesNotExist>()
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
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

        CoreModel.moveFile(config, folder.id, document.id)
            .unwrapErrorType<MoveFileError.DocumentTreatedAsFolder>()
    }

    @Test
    fun moveFileTargetParentDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.moveFile(config, document.id, generateId())
            .unwrapErrorType<MoveFileError.TargetParentDoesNotExist>()
    }

    @Test
    fun moveFileTargetParentHasChildNamedThat() {
        val documentName = generateAlphaString()

        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        val firstDocument = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            documentName,
            FileType.Document
        ).unwrap()

        val secondDocument = CoreModel.createFile(
            config,
            folder.id,
            documentName,
            FileType.Document
        ).unwrap()

        CoreModel.moveFile(config, firstDocument.id, folder.id)
            .unwrapErrorType<MoveFileError.TargetParentHasChildNamedThat>()
    }

    @Test
    fun cannotMoveRoot() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        CoreModel.moveFile(config, rootFileMetadata.id, rootFileMetadata.id)
            .unwrapErrorType<MoveFileError.CannotMoveRoot>()
    }

    @Test
    fun moveFileMoveFolderIntoItself() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.moveFile(config, folder.id, folder.id)
            .unwrapErrorType<MoveFileError.FolderMovedIntoItself>()
    }

    @Test
    fun moveFileUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<Unit, MoveFileError>>(
            moveFile("", "", "")
        ).unwrapUnexpected()
    }
}
