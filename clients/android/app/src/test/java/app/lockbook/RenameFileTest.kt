package app.lockbook

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
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<Unit>(
            CoreModel.renameFile(config, document.id, generateAlphaString()).component1()
        )

        assertType<Unit>(
            CoreModel.renameFile(config, folder.id, generateAlphaString()).component1()
        )
    }

    @Test
    fun renameFileDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<RenameFileError.FileDoesNotExist>(
            CoreModel.renameFile(config, generateId(), generateAlphaString()).component2()
        )
    }

    @Test
    fun renameFileContainsSlash() {
        val fileName = generateAlphaString()
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                fileName,
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<RenameFileError.FileNameNotAvailable>(
            CoreModel.renameFile(config, document.id, fileName).component2()
        )
    }

    @Test
    fun renameFileEmpty() {
        val fileName = generateAlphaString()
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                fileName,
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<RenameFileError.NewNameEmpty>(
            CoreModel.renameFile(config, document.id, "").component2()
        )
    }

    @Test
    fun cannotRenameRoot() {
        val fileName = generateAlphaString()
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<RenameFileError.CannotRenameRoot>(
            CoreModel.renameFile(config, rootFileMetadata.id, "not_root").component2()
        )
    }

    @Test
    fun renameFileUnexpectedError() {
        val renameFileResult: Result<Unit, RenameFileError>? =
            Klaxon().converter(getUsageConverters).parse(renameFile("", "", ""))

        assertType<RenameFileError.UnexpectedError>(
            renameFileResult?.component2()
        )
    }
}
