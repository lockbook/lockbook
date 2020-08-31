package app.lockbook

import app.lockbook.core.deleteFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.*

@Ignore("Delete endpoint doesn't work yet")
class DeleteFileTest {
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
    fun deleteFileOk() {
        assertType<Unit>(
            this::deleteFileOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::deleteFileOk.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::deleteFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::deleteFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::deleteFileOk.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::deleteFileOk.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<Unit>(
            this::deleteFileOk.name,
            CoreModel.deleteFile(config, document.id).component1()
        )

        assertType<Unit>(
            this::deleteFileOk.name,
            CoreModel.deleteFile(config, folder.id).component1()
        )
    }

    @Test
    fun deleteFileNoFileWithThatId() {
        assertType<Unit>(
            this::deleteFileNoFileWithThatId.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<DeleteFileError.NoFileWithThatId>(
            this::deleteFileNoFileWithThatId.name,
            CoreModel.deleteFile(config, generateId()).component2()
        )
    }

    @Test
    fun deleteFileUnexpectedError() {
        val deleteFile: Result<Unit, DeleteFileError>? =
            Klaxon().converter(deleteFileConverter).parse(deleteFile("", ""))

        assertType<DeleteFileError.UnexpectedError>(
            this::deleteFileUnexpectedError.name,
            deleteFile?.component2()
        )
    }
}
