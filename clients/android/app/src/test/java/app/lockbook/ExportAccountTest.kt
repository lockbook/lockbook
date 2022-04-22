package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.AccountExportError
import app.lockbook.util.Config
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class ExportAccountTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, createRandomPath()))
    }

    @Test
    fun exportAccountOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.exportAccount().unwrapOk()
    }

    @Test
    fun exportAccountNoAccount() {
        CoreModel.exportAccount().unwrapErrorType(AccountExportError.NoAccount)
    }
}
