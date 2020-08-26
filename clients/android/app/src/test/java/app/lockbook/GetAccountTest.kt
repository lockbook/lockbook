package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.GetAccountError
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetAccountTest {

    private val coreModel = CoreModel(Config(path))

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
        }
    }

    @Before
    fun createDirectory() {
        Runtime.getRuntime().exec("mkdir $path")
        Runtime.getRuntime().exec("rm -rf $path")
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path")
    }

    @Test
    fun getAccountOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.getAccount().component1()!!
    }

    @Test
    fun getAccountNoAccount() {
        val getAccountError = coreModel.getAccount().component2()!!
        require(getAccountError is GetAccountError.NoAccount)
    }
}