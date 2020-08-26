package app.lockbook

import app.lockbook.core.getAccount
import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetAccountTest {

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
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.getAccount().component1()!!
    }

    @Test
    fun getAccountNoAccount() {
        val coreModel = CoreModel(Config(path))
        val getAccountError = coreModel.getAccount().component2()!!
        require(getAccountError is GetAccountError.NoAccount)
    }

    @Test
    fun getAccountUnexpectedError() {
        val getAccountResult: Result<Account, GetAccountError>? =
            Klaxon().converter(getAccountConverter).parse(getAccount(""))
        val getAccountError = getAccountResult!!.component2()!!
        require(getAccountError is GetAccountError.UnexpectedError)
    }
}
