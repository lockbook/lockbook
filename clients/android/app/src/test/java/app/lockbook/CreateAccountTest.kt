package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.CreateAccountError
import org.junit.Before
import org.junit.Test


class CreateAccountTest {

    @Before
    fun loadLib() {
        loadLockbookCore()
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

        val secondAccountError = CoreModel.generateAccount(
            Config(path),
            username
        ).component2()!!
        require(secondAccountError is CreateAccountError.UsernameTaken)
    }
}