package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.GetAccountError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetAccountTest {
    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @Before
    fun createDirectory() {
        CoreModel.init(Config(false, createRandomPath()))
    }

    @Test
    fun getAccountOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.getAccount().unwrapOk()
    }

    @Test
    fun getAccountNoAccount() {
        CoreModel.getAccount().unwrapErrorType(GetAccountError.NoAccount)
    }
}
