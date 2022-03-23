package app.lockbook

import app.lockbook.core.exportAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.AccountExportError
import app.lockbook.util.Config
import app.lockbook.util.IntermCoreResult
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ExportAccountTest {
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
    fun exportAccountOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.exportAccount(config).unwrapOk()
    }

    @Test
    fun exportAccountNoAccount() {
        CoreModel.exportAccount(config).unwrapErrorType(AccountExportError.NoAccount)
    }

    @Test
    fun exportAccountUnexpectedError() {
        CoreModel.exportAccountParser.decodeFromString<IntermCoreResult<String, AccountExportError>>(
            exportAccount("")
        ).unwrapUnexpected()
    }
}
