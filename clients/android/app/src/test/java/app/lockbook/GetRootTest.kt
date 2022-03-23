package app.lockbook

import app.lockbook.core.getRoot
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.DecryptedFileMetadata
import app.lockbook.util.GetRootError
import app.lockbook.util.IntermCoreResult
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetRootTest {
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
    fun getRootOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getRoot(config).unwrapOk()
    }

    @Test
    fun getRootUnexpectedError() {
        CoreModel.getRootParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, GetRootError>>(
            getRoot("")
        ).unwrapUnexpected()
    }
}
