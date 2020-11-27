package app.lockbook

import app.lockbook.core.deleteFile
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.*

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
            CoreModel.deleteFile(config, document.id).component1()
        )

        assertType<Unit>(
            CoreModel.deleteFile(config, folder.id).component1()
        )
    }

    @Test
    fun deleteFileNoFileWithThatId() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<FileDeleteError.FileDoesNotExist>(
            CoreModel.deleteFile(config, generateId()).component2()
        )
    }

    @Test
    fun deleteFileCannotDeleteRoot() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<FileDeleteError.CannotDeleteRoot>(
            CoreModel.deleteFile(config, rootFileMetadata.id).component2()
        )
    }

    @Test
    fun deleteFileUnexpectedError() {
        val fileDelete: Result<Unit, FileDeleteError>? =
            Klaxon().converter(deleteFileConverter).parse(deleteFile("", ""))

        assertType<FileDeleteError.Unexpected>(
            fileDelete?.component2()
        )
    }
}
