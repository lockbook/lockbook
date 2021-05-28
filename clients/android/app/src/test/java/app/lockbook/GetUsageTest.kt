package app.lockbook

import app.lockbook.core.getUsageHumanString
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
    fun getUsageHumanStringOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<String>(
            CoreModel.getUsageHumanString(config, true).component1()
        )

        assertType<String>(
            CoreModel.getUsageHumanString(config, false).component1()
        )
    }

    @Test
    fun getUsageHumanStringNoAccount() {
        assertType<String>(
            CoreModel.getUsageHumanString(config, true).component1()
        )

        assertType<String>(
            CoreModel.getUsageHumanString(config, false).component1()
        )
    }

    @Test
    fun getUsageHumanStringUnexpectedError() {
        assertType<GetAccountError.Unexpected>(
            Klaxon().converter(getUsageHumanStringConverter).parse<Result<Account, GetAccountError>>(
                getUsageHumanString("", false)
            )?.component2()
        )

        assertType<GetAccountError.Unexpected>(
            Klaxon().converter(getUsageHumanStringConverter).parse<Result<Account, GetAccountError>>(
                getUsageHumanString("", true)
            )?.component2()
        )
    }
}
