package app.lockbook

import app.lockbook.core.backgroundSync
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.FileType
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.SyncAllError
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class SyncAllTest {
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
    fun syncAllOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.syncAll(config, null).unwrapOk()
    }

    @Test
    fun syncAllNoAccount() {
        CoreModel.syncAll(config, null).unwrapErrorType(SyncAllError.NoAccount)
    }

    @Test
    fun syncAllUnexpectedError() {
        CoreModel.syncAllParser.decodeFromString<IntermCoreResult<Unit, SyncAllError>>(
            backgroundSync("")
        ).unwrapUnexpected()
    }
}
