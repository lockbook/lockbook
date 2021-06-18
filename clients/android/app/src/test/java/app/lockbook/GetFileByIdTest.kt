package app.lockbook

import app.lockbook.core.exportAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
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

        assertType<ClientFileMetadata>(
            CoreModel.getFileById(config, document.id).component1()
        )

        assertType<ClientFileMetadata>(
            CoreModel.getFileById(config, folder.id).component1()
        )
    }

    @Test
    fun getFileByIdNoFile() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<GetFileByIdError.NoFileWithThatId>(
            CoreModel.getFileById(config, generateId()).component2()
        )
    }

    @Test
    fun getFileByIdUnexpectedError() {
        assertType<GetFileByIdError.Unexpected>(
            Klaxon().converter(getFileByIdConverter)
                .parse<Result<ClientFileMetadata, GetFileByIdError>>(exportAccount(""))?.component2()
        )
    }
}
