package app.lockbook

import app.lockbook.core.getAccount
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<Account>(
            CoreModel.getAccount(config).component1()
        )
    }

    @Test
    fun getAccountNoAccount() {
        assertType<GetAccountError.NoAccount>(
            CoreModel.getAccount(config).component2()
        )
    }

    @Test
    fun getAccountUnexpectedError() {
        assertType<GetAccountError.Unexpected>(
            Klaxon().converter(getAccountConverter).parse<Result<Account, GetAccountError>>(getAccount(""))?.component2()
        )
    }
}
