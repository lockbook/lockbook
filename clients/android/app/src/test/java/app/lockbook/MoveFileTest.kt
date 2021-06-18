package app.lockbook

import app.lockbook.core.moveFile
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.moveFile(config, document.id, folder.id).component1()
        )
    }

    @Test
    fun moveFileDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<MoveFileError.FileDoesNotExist>(
            CoreModel.moveFile(config, generateId(), folder.id).component2()
        )
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<MoveFileError.DocumentTreatedAsFolder>(
            CoreModel.moveFile(config, folder.id, document.id).component2()
        )
    }

    @Test
    fun moveFileTargetParentDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<MoveFileError.TargetParentDoesNotExist>(
            CoreModel.moveFile(config, document.id, generateId()).component2()
        )
    }

    @Test
    fun moveFileTargetParentHasChildNamedThat() {
        val documentName = generateAlphaString()

        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        val firstDocument = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                documentName,
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val secondDocument = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                folder.id,
                documentName,
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<MoveFileError.TargetParentHasChildNamedThat>(
            CoreModel.moveFile(config, firstDocument.id, folder.id).component2()
        )
    }

    @Test
    fun cannotMoveRoot() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<MoveFileError.CannotMoveRoot>(
            CoreModel.moveFile(config, rootFileMetadata.id, rootFileMetadata.id).component2()
        )
    }

    @Test
    fun moveFileMoveFolderIntoItself() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<MoveFileError.FolderMovedIntoItself>(
            CoreModel.moveFile(config, folder.id, folder.id).component2()
        )
    }

    @Test
    fun moveFileUnexpectedError() {
        assertType<MoveFileError.Unexpected>(
            Klaxon().converter(moveFileConverter).parse<Result<Unit, MoveFileError>>(moveFile("", "", ""))?.component2()
        )
    }
}
