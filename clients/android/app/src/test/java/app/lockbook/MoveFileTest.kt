package app.lockbook

import app.lockbook.core.moveFile
import app.lockbook.utils.*
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
            this::moveFileOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::moveFileOk.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::moveFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::moveFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::moveFileOk.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::moveFileOk.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<Unit>(
            this::moveFileOk.name,
            CoreModel.moveFile(config, document.id, folder.id).component1()
        )
    }

    @Test
    fun moveFileDoesNotExist() {
        assertType<Unit>(
            this::moveFileDoesNotExist.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::moveFileDoesNotExist.name,
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::moveFileDoesNotExist.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::moveFileDoesNotExist.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<MoveFileError.FileDoesNotExist>(
            this::moveFileDoesNotExist.name,
            CoreModel.moveFile(config, generateId(), folder.id).component2()
        )
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
        assertType<Unit>(
            this::moveFileDocumentTreatedAsFolder.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::moveFileDocumentTreatedAsFolder.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::moveFileDocumentTreatedAsFolder.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::moveFileDocumentTreatedAsFolder.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::moveFileDocumentTreatedAsFolder.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::moveFileDocumentTreatedAsFolder.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<MoveFileError.DocumentTreatedAsFolder>(
            this::moveFileDocumentTreatedAsFolder.name,
            CoreModel.moveFile(config, folder.id, document.id).component2()
        )
    }

    @Test
    fun moveFileTargetParentDoesNotExist() {
        assertType<Unit>(
            this::moveFileTargetParentDoesNotExist.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::moveFileTargetParentDoesNotExist.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::moveFileTargetParentDoesNotExist.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<Unit>(
            this::moveFileTargetParentDoesNotExist.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<MoveFileError.TargetParentDoesNotExist>(
            this::moveFileTargetParentDoesNotExist.name,
            CoreModel.moveFile(config, document.id, generateId()).component2()
        )
    }

    @Test
    fun moveFileTargetParentHasChildNamedThat() {
        val documentName = generateAlphaString()

        assertType<Unit>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        val firstDocument = assertTypeReturn<FileMetadata>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                documentName,
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val secondDocument = assertTypeReturn<FileMetadata>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.createFile(
                config,
                folder.id,
                documentName,
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<Unit>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<Unit>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.insertFile(config, firstDocument).component1()
        )

        assertType<Unit>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.insertFile(config, secondDocument).component1()
        )

        assertType<MoveFileError.TargetParentHasChildNamedThat>(
            this::moveFileTargetParentHasChildNamedThat.name,
            CoreModel.moveFile(config, firstDocument.id, folder.id).component2()
        )
    }

    @Test
    fun moveFileUnexpectedError() {
        val moveResult: Result<Unit, MoveFileError>? =
            Klaxon().converter(moveFileConverter).parse(moveFile("", "", ""))

        assertType<MoveFileError.UnexpectedError>(
            this::moveFileUnexpectedError.name,
            moveResult?.component2()
        )
    }
}
