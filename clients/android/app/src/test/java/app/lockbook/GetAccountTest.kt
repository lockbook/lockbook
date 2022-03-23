package app.lockbook

import app.lockbook.core.getAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.Account
import app.lockbook.util.Config
import app.lockbook.util.GetAccountError
import app.lockbook.util.IntermCoreResult
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetAccountTest {
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
    fun getAccountOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getAccount(config).unwrapOk()
    }

    @Test
    fun getAccountNoAccount() {
        CoreModel.getAccount(config).unwrapErrorType(GetAccountError.NoAccount)
    }

    @Test
    fun getAccountUnexpectedError() {
        CoreModel.getAccountParser.decodeFromString<IntermCoreResult<Account, GetAccountError>>(
            getAccount("")
        ).unwrapUnexpected()
    }
}
