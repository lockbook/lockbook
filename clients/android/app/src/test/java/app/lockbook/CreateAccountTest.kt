package app.lockbook

import app.lockbook.core.createAccount
import app.lockbook.model.CoreModel
import app.lockbook.model.CoreModel.getAPIURL
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import kotlinx.serialization.*
import kotlinx.serialization.json.Json
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test
import kotlinx.serialization.json.*
import kotlinx.serialization.modules.*
import kotlinx.serialization.PolymorphicSerializer.*


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
        CoreModel.createAccount(config, generateAlphaString()).unwrap()
    }

    @Test
    fun createAccountUsernameTaken() {
        val username = generateAlphaString()

        CoreModel.createAccount(config, username).unwrap()

        config = Config(createRandomPath())

        CoreModel.createAccount(config, username)
            .unwrapErrorType<CreateAccountError.UsernameTaken>()
    }

    @Test
    fun createAccountInvalidUsername() {
        CoreModel.createAccount(config, "!@#$%^&*()")
            .unwrapErrorType<CreateAccountError.InvalidUsername>()
    }

    @Test
    fun createAccountExistsAlready() {
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        CoreModel.createAccount(config, generateAlphaString())
            .unwrapErrorType<CreateAccountError.AccountExistsAlready>()
    }

    @Test
    fun createAccountUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<Unit, CreateAccountError>>(
            createAccount("", "", getAPIURL())
        ).unwrapUnexpected()
    }
}
