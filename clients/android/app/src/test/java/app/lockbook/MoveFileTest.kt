package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.MoveFileError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class MoveFileTest {

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
    fun moveFileOk() {
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

        CoreModel.moveFile(document.id, folder.id).unwrapOk()
    }

    @Test
    fun moveFileDoesNotExist() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.moveFile(generateId(), folder.id)
            .unwrapErrorType(MoveFileError.FileDoesNotExist)
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
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

        CoreModel.moveFile(folder.id, document.id)
            .unwrapErrorType(MoveFileError.DocumentTreatedAsFolder)
    }

    @Test
    fun moveFileTargetParentDoesNotExist() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val document = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.moveFile(document.id, generateId())
            .unwrapErrorType(MoveFileError.TargetParentDoesNotExist)
    }

    @Test
    fun moveFileTargetParentHasChildNamedThat() {

        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        val documentName = generateAlphaString()

        val firstDocument = CoreModel.createFile(
            rootFileMetadata.id,
            documentName,
            FileType.Document
        ).unwrapOk()

        CoreModel.createFile(
            folder.id,
            documentName,
            FileType.Document
        ).unwrapOk()

        CoreModel.moveFile(firstDocument.id, folder.id)
            .unwrapErrorType(MoveFileError.TargetParentHasChildNamedThat)
    }

    @Test
    fun cannotMoveRoot() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.moveFile(rootFileMetadata.id, folder.id)
            .unwrapErrorType(MoveFileError.CannotMoveRoot)
    }

    @Test
    fun moveFileMoveFolderIntoItself() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        val folder = CoreModel.createFile(
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.moveFile(folder.id, folder.id)
            .unwrapErrorType(MoveFileError.FolderMovedIntoItself)
    }
}
