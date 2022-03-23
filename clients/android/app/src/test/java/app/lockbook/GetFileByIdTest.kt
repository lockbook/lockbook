package app.lockbook

import app.lockbook.core.exportAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import kotlinx.serialization.decodeFromString
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
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.getFileById(config, document.id).unwrapOk()

        CoreModel.getFileById(config, folder.id).unwrapOk()
    }

    @Test
    fun getFileByIdNoFile() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getFileById(config, generateId())
            .unwrapErrorType(GetFileByIdError.NoFileWithThatId)
    }

    @Test
    fun getFileByIdUnexpectedError() {
        CoreModel.getFileByIdParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, GetFileByIdError>>(
            exportAccount("")
        ).unwrapUnexpected()
    }
}
