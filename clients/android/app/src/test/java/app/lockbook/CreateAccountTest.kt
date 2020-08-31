package app.lockbook

import app.lockbook.core.createAccount
import app.lockbook.utils.*
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
            this::createAccountOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )
    }

    @Test
    fun createAccountUsernameTaken() {
        val username = generateAlphaString()
        assertType<Unit>(
            this::createAccountUsernameTaken.name,
            CoreModel.generateAccount(config, username).component1()
        )
        config = Config(createRandomPath())

        assertType<CreateAccountError.UsernameTaken>(
            this::createAccountUsernameTaken.name,
            CoreModel.generateAccount(config, username).component2()
        )
    }

    @Test
    fun createAccountInvalidUsername() {
        assertType<CreateAccountError.InvalidUsername>(
            this::createAccountInvalidUsername.name,
            CoreModel.generateAccount(config, "!@#$%^&*()").component2()
        )
    }

    @Test
    fun createAccountExistsAlready() {
        assertType<Unit>(
            this::createAccountExistsAlready.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )
        assertType<CreateAccountError.AccountExistsAlready>(
            this::createAccountExistsAlready.name,
            CoreModel.generateAccount(config, generateAlphaString()).component2()
        )
    }

    @Test
    fun createAccountUnexpectedError() {
        val createAccountOk: Result<Unit, CreateAccountError>? =
            Klaxon().converter(createAccountConverter)
                .parse(createAccount("", ""))

        assertType<CreateAccountError.UnexpectedError>(
            this::createAccountUnexpectedError.name,
            createAccountOk?.component2()
        )
    }
}
