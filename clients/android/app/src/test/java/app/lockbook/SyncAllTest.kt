package app.lockbook

import app.lockbook.core.backgroundSync
import app.lockbook.core.saveDocumentToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.syncAll(config, null).unwrap()
    }

    @Test
    fun syncAllNoAccount() {
        CoreModel.syncAll(config, null).unwrapErrorType<SyncAllError.NoAccount>()
    }

    @Test
    fun syncAllUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<Unit, SyncAllError>>(
            backgroundSync("")
        ).unwrapUnexpected()
    }
}
