package app.lockbook

import app.lockbook.core.getLocalAndServerUsage
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetUsageTest {
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
    fun getLocalAndServerUsageOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<String>(
            CoreModel.getLocalAndServerUsage(config, true).component1()
        )

        assertType<String>(
            CoreModel.getLocalAndServerUsage(config, false).component1()
        )
    }

    @Test
    fun getLocalAndServerUsageNoAccount() {
        assertType<String>(
            CoreModel.getLocalAndServerUsage(config, true).component1()
        )

        assertType<String>(
            CoreModel.getLocalAndServerUsage(config, false).component1()
        )
    }

    @Test
    fun getLocalAndServerUsageUnexpectedError() {
        assertType<GetAccountError.Unexpected>(
            Klaxon().converter(getLocalAndServerUsageConverter).parse<Result<Account, GetAccountError>>(
                getLocalAndServerUsage("", false)
            )?.component2()
        )

        assertType<GetAccountError.Unexpected>(
            Klaxon().converter(getLocalAndServerUsageConverter).parse<Result<Account, GetAccountError>>(
                getLocalAndServerUsage("", true)
            )?.component2()
        )
    }
}
