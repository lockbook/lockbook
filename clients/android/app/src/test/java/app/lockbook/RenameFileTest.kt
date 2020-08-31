package app.lockbook

import app.lockbook.core.moveFile
import app.lockbook.core.renameFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            this::renameFileOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::renameFileOk.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::renameFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::renameFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::renameFileOk.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::renameFileOk.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<Unit>(
            this::renameFileOk.name,
            CoreModel.renameFile(config, document.id, generateAlphaString()).component1()
        )

        assertType<Unit>(
            this::renameFileOk.name,
            CoreModel.renameFile(config, folder.id, generateAlphaString()).component1()
        )
    }

    @Test
    fun renameFileDoesNotExist() {
        assertType<Unit>(
            this::renameFileDoesNotExist.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertTypeReturn<FileMetadata>(
            this::renameFileDoesNotExist.name,
            CoreModel.getRoot(config).component1()
        )

        assertType<RenameFileError.FileDoesNotExist>(
            this::renameFileDoesNotExist.name,
            CoreModel.renameFile(config, generateId(), generateAlphaString()).component2()
        )
    }

    @Test
    fun renameFileContainsSlash() {
        val fileName = generateAlphaString()
        assertType<Unit>(
            this::renameFileContainsSlash.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::renameFileContainsSlash.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::renameFileContainsSlash.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::renameFileContainsSlash.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                fileName,
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::renameFileContainsSlash.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::renameFileContainsSlash.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<RenameFileError.FileNameNotAvailable>(
            this::renameFileContainsSlash.name,
            CoreModel.renameFile(config, document.id, fileName).component2()
        )
    }

    @Test
    fun renameFileUnexpectedError() {
        val renameFileResult: Result<Unit, RenameFileError>? =
            Klaxon().converter(renameFileConverter).parse(renameFile("", "", ""))

        assertType<RenameFileError.UnexpectedError>(
            this::renameFileUnexpectedError.name,
            renameFileResult?.component2()
        )
    }
}