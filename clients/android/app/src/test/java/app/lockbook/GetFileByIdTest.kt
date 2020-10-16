package app.lockbook

import app.lockbook.core.exportAccount
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetFileByIdTest {
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
    fun getFileByIdOk() {
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

        assertType<FileMetadata>(
            CoreModel.getFileById(config, document.id).component1()
        )

        assertType<FileMetadata>(
            CoreModel.getFileById(config, folder.id).component1()
        )
    }

    @Test
    fun getFileByIdNoFile() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<CoreError.NoFileWithThatId>(
            CoreModel.getFileById(config, generateId()).component2()
        )
    }

    @Test
    fun getFileByIdUnexpectedError() {
        val getFileByIdResult: Result<FileMetadata, CoreError>? =
            Klaxon().converter(getFileByIdConverter)
                .parse(exportAccount(""))

        assertType<CoreError.Unexpected>(
            getFileByIdResult?.component2()
        )
    }
}
