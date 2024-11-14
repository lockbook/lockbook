package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.ImportError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class ImportAccountTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lb_external_interface")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, false, createRandomPath()))
    }

    @Test
    fun importAccountOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val exportAccountString = CoreModel.exportAccount().unwrapOk()

        CoreModel.init(Config(false, false, createRandomPath()))

        CoreModel.importAccount(exportAccountString).unwrapOk()
    }

    @Test
    fun importAccountStringCorrupted() {
        CoreModel.importAccount("!@#$%^&*()")
            .unwrapErrorType(ImportError.AccountStringCorrupted)
    }
}
