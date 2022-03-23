package app.lockbook

import app.lockbook.core.importAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.ImportError
import app.lockbook.util.IntermCoreResult
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
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val exportAccountString = CoreModel.exportAccount(config).unwrapOk()

        config = Config(createRandomPath())

        CoreModel.importAccount(config, exportAccountString).unwrapOk()
    }

    @Test
    fun importAccountStringCorrupted() {
        CoreModel.importAccount(config, "!@#$%^&*()")
            .unwrapErrorType(ImportError.AccountStringCorrupted)
    }

    @Test
    fun importAccountUnexpectedError() {
        CoreModel.importAccountParser.decodeFromString<IntermCoreResult<Unit, ImportError>>(
            importAccount("", "")
        ).unwrapUnexpected()
    }
}
