package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.CreateAccountError
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class CreateAccountTest {
    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
            Runtime.getRuntime().exec("rm -rf $path")
        }
    }

    @Before
    fun createDirectory() {
        Runtime.getRuntime().exec("mkdir $path")
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path")
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
        Runtime.getRuntime().exec("rm -rf $path")
        Runtime.getRuntime().exec("mkdir $path")

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
        val secondCreateAccountError = CoreModel.generateAccount(
            Config(path),
            "œ∑´´†¥¨ˆˆπåß∂ƒ"
        ).component2()!!
        require(secondCreateAccountError is CreateAccountError.InvalidUsername)
        val thirdCreateAccountError = CoreModel.generateAccount(
            Config(path),
            "Ω≈ç√∫˜˜¬˚∆˙©"
        ).component2()!!
        require(thirdCreateAccountError is CreateAccountError.InvalidUsername)
        val fourthCreateAccountError = CoreModel.generateAccount(
            Config(path),
            "☺️☠️✋☝️✊"
        ).component2()!!
        require(fourthCreateAccountError is CreateAccountError.InvalidUsername)
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
}
