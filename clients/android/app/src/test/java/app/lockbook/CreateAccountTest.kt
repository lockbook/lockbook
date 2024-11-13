package app.lockbook

import app.lockbook.util.Config
import app.lockbook.util.CreateAccountError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class CreateAccountTest {

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
    fun createAccountOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()
    }

    @Test
    fun createAccountUsernameTaken() {
        val username = generateAlphaString()

        CoreModel.createAccount(username).unwrapOk()

        CoreModel.init(Config(false, false, createRandomPath()))

        CoreModel.createAccount(username)
            .unwrapErrorType(CreateAccountError.UsernameTaken)
    }

    @Test
    fun createAccountInvalidUsername() {
        CoreModel.createAccount("!@#$%^&*()")
            .unwrapErrorType(CreateAccountError.InvalidUsername)
    }

    @Test
    fun createAccountExistsAlready() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.createAccount(generateAlphaString())
            .unwrapErrorType(CreateAccountError.AccountExistsAlready)
    }
}
