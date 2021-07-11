package app.lockbook

import app.lockbook.core.exportAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.getFileById(config, document.id).unwrap()

        CoreModel.getFileById(config, folder.id).unwrap()
    }

    @Test
    fun getFileByIdNoFile() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.getFileById(config, generateId())
            .unwrapErrorType<GetFileByIdError.NoFileWithThatId>()
    }

    @Test
    fun getFileByIdUnexpectedError() {
        Klaxon().converter(getFileByIdConverter)
            .parse<Result<ClientFileMetadata, GetFileByIdError>>(exportAccount(""))
            .unwrapErrorType<GetFileByIdError.Unexpected>()
    }
}
