package app.lockbook

import app.lockbook.core.getUncompressedUsage
import app.lockbook.core.importAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ImportAccountTest {
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
    fun importAccountOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val exportAccountString = CoreModel.exportAccount(config).unwrap()

        config = Config(createRandomPath())

        CoreModel.importAccount(config, exportAccountString).unwrap()
    }

    @Test
    fun importAccountStringCorrupted() {
        CoreModel.importAccount(config, "!@#$%^&*()")
            .unwrapErrorType(ImportError.AccountStringCorrupted)
    }

    @Test
    fun importAccountUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<Unit, ImportError>>(
            importAccount("", "")
        ).unwrapUnexpected()
    }
}
