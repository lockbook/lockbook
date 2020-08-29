package app.lockbook

import app.lockbook.core.createAccount
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.CreateAccountError
import app.lockbook.utils.createAccountConverter
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class CreateAccountTest {
    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun createAccountOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
    }

    @Test
    fun createAccountUsernameTaken() {
        val username = generateAlphaString()
        CoreModel.generateAccount(Config(path), username).component1()!!
        path = createRandomPath()

        val secondAccountError = CoreModel.generateAccount(Config(path), username).component2()!!
        require(secondAccountError is CreateAccountError.UsernameTaken)
    }

    @Test
    fun createAccountInvalidUsername() {
        val firstCreateAccountError = CoreModel.generateAccount(
            Config(path),
            "!@#$%^&*()"
        ).component2()!!
        require(firstCreateAccountError is CreateAccountError.InvalidUsername)
    }

    @Test
    fun createAccountExistsAlready() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        val createAccountError =
            CoreModel.generateAccount(Config(path), generateAlphaString()).component2()!!
        require(createAccountError is CreateAccountError.AccountExistsAlready)
    }

    @Test
    fun createAccountUnexpectedError() {
        val createAccountResult: Result<Unit, CreateAccountError>? =
            Klaxon().converter(createAccountConverter)
                .parse(createAccount("", ""))
        val createAccountError = createAccountResult!!.component2()!!
        require(createAccountError is CreateAccountError.UnexpectedError)
    }
}
