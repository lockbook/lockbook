package app.lockbook

import app.lockbook.core.moveFile
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.MoveFileError
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

        CoreModel.moveFile(config, document.id, folder.id).unwrapOk()
    }

    @Test
    fun moveFileDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.moveFile(config, generateId(), folder.id)
            .unwrapErrorType(MoveFileError.FileDoesNotExist)
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
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

        CoreModel.moveFile(config, folder.id, document.id)
            .unwrapErrorType(MoveFileError.DocumentTreatedAsFolder)
    }

    @Test
    fun moveFileTargetParentDoesNotExist() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.moveFile(config, document.id, generateId())
            .unwrapErrorType(MoveFileError.TargetParentDoesNotExist)
    }

    @Test
    fun moveFileTargetParentHasChildNamedThat() {
        val documentName = generateAlphaString()

        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        val firstDocument = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            documentName,
            FileType.Document
        ).unwrapOk()

        val secondDocument = CoreModel.createFile(
            config,
            folder.id,
            documentName,
            FileType.Document
        ).unwrapOk()

        CoreModel.moveFile(config, firstDocument.id, folder.id)
            .unwrapErrorType(MoveFileError.TargetParentHasChildNamedThat)
    }

    @Test
    fun cannotMoveRoot() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        CoreModel.moveFile(config, rootFileMetadata.id, rootFileMetadata.id)
            .unwrapErrorType(MoveFileError.CannotMoveRoot)
    }

    @Test
    fun moveFileMoveFolderIntoItself() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.moveFile(config, folder.id, folder.id)
            .unwrapErrorType(MoveFileError.FolderMovedIntoItself)
    }

    @Test
    fun moveFileUnexpectedError() {
        CoreModel.moveFileParser.decodeFromString<IntermCoreResult<Unit, MoveFileError>>(
            moveFile("", "", "")
        ).unwrapUnexpected()
    }
}
