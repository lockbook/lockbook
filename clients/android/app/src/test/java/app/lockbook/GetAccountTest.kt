package app.lockbook

import app.lockbook.core.getAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.getAccount(config).unwrap()
    }

    @Test
    fun getAccountNoAccount() {
        CoreModel.getAccount(config).unwrapErrorType<GetAccountError.NoAccount>()
    }

    @Test
    fun getAccountUnexpectedError() {
        Klaxon().converter(getAccountConverter)
            .parse<Result<Account, GetAccountError>>(getAccount(""))
            .unwrapErrorType<GetAccountError.Unexpected>()
    }
}
