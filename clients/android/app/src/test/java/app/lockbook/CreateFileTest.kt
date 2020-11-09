package app.lockbook

import app.lockbook.core.createFile
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class CreateFileTest {
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
    fun createFileOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )
    }

    @Test
    fun createFileContainsSlash() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<CreateFileError.FileNameContainsSlash>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                "/",
                Klaxon().toJsonString(FileType.Document)
            ).component2()
        )

        assertType<CreateFileError.FileNameContainsSlash>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                "/",
                Klaxon().toJsonString(FileType.Folder)
            ).component2()
        )
    }

    @Test
    fun createFileEmpty() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<CreateFileError.FileNameEmpty>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                "",
                Klaxon().toJsonString(FileType.Document)
            ).component2()
        )

        assertType<CreateFileError.FileNameEmpty>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                "",
                Klaxon().toJsonString(FileType.Folder)
            ).component2()
        )
    }

    @Test
    fun createFileNotAvailable() {
        val fileName = generateAlphaString()

        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                fileName,
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<CreateFileError.FileNameNotAvailable>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                fileName,
                Klaxon().toJsonString(FileType.Folder)
            ).component2()
        )
    }

    @Test
    fun createFileNoAccount() {
        assertType<CreateFileError.NoAccount>(
            CoreModel.createFile(
                config,
                generateId(),
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component2()
        )
    }

    @Test
    fun createFileUnexpectedError() {
        val createFileResult: Result<FileMetadata, CreateFileError>? =
            Klaxon().converter(createFileConverter)
                .parse(createFile("", "", "", ""))

        assertType<CreateFileError.Unexpected>(
            createFileResult?.component2()
        )
    }
}
