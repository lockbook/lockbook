package app.lockbook

import app.lockbook.core.createFile
import app.lockbook.core.getRoot
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        CoreModel.getRoot(config).unwrap()
    }

    @Test
    fun getRootUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, GetRootError>>(
            getRoot("")
        ).unwrapUnexpected()
    }
}
