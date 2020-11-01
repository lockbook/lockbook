package app.lockbook

import app.lockbook.core.createAccount
import app.lockbook.utils.*
import app.lockbook.utils.CoreModel.API_URL
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )
    }

    @Test
    fun createAccountUsernameTaken() {
        val username = generateAlphaString()
        assertType<Unit>(
            CoreModel.generateAccount(config, username).component1()
        )
        config = Config(createRandomPath())

        assertType<CreateAccountError.UsernameTaken>(
            CoreModel.generateAccount(config, username).component2()
        )
    }

    @Test
    fun createAccountInvalidUsername() {
        assertType<CreateAccountError.InvalidUsername>(
            CoreModel.generateAccount(config, "!@#$%^&*()").component2()
        )
    }

    @Test
    fun createAccountExistsAlready() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )
        assertType<CreateAccountError.AccountExistsAlready>(
            CoreModel.generateAccount(config, generateAlphaString()).component2()
        )
    }

    @Test
    fun createAccountUnexpectedError() {
        val createAccountOk: Result<Unit, CreateAccountError>? =
            Klaxon().converter(createAccountConverter)
                .parse(createAccount("", "", API_URL))

        assertType<CreateAccountError.Unexpected>(
            createAccountOk?.component2()
        )
    }
}
