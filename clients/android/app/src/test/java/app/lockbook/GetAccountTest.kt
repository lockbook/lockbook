package app.lockbook

import app.lockbook.core.getAccount
import app.lockbook.utils.*
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
            this::getAccountOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<Account>(
            this::getAccountOk.name,
            CoreModel.getAccount(config).component1()
        )
    }

    @Test
    fun getAccountNoAccount() {
        assertType<GetAccountError.NoAccount>(
            this::getAccountNoAccount.name,
            CoreModel.getAccount(config).component2()
        )
    }

    @Test
    fun getAccountUnexpectedError() {
        val getAccountResult: Result<Account, GetAccountError>? =
            Klaxon().converter(getAccountConverter).parse(getAccount(""))

        assertType<GetAccountError.UnexpectedError>(
            this::getAccountUnexpectedError.name,
            getAccountResult?.component2()
        )
    }
}
