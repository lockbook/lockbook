package app.lockbook

import app.lockbook.core.createAccount
import app.lockbook.model.CoreModel
import app.lockbook.model.CoreModel.getAPIURL
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class CreateAccountTest {
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
    fun createAccountOk() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()
    }

    @Test
    fun createAccountUsernameTaken() {
        val username = generateAlphaString()

        CoreModel.generateAccount(config, username).unwrap()

        config = Config(createRandomPath())

        CoreModel.generateAccount(config, username)
            .unwrapErrorType<CreateAccountError.UsernameTaken>()
    }

    @Test
    fun createAccountInvalidUsername() {
        CoreModel.generateAccount(config, "!@#$%^&*()")
            .unwrapErrorType<CreateAccountError.InvalidUsername>()
    }

    @Test
    fun createAccountExistsAlready() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.generateAccount(config, generateAlphaString())
            .unwrapErrorType<CreateAccountError.AccountExistsAlready>()
    }

    @Test
    fun createAccountUnexpectedError() {
        Klaxon().converter(createAccountConverter)
            .parse<Result<Unit, CreateAccountError>>(createAccount("", "", getAPIURL()))
            .unwrapErrorType<CreateAccountError.Unexpected>()
    }
}
