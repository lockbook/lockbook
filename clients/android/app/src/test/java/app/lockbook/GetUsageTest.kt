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

        assertType<LocalAndServerUsages>(
            CoreModel.getLocalAndServerUsage(config, true).component1()
        )

        assertType<LocalAndServerUsages>(
            CoreModel.getLocalAndServerUsage(config, false).component1()
        )
    }

    @Test
    fun getLocalAndServerUsageNoAccount() {
        assertType<GetUsageError.NoAccount>(
            CoreModel.getLocalAndServerUsage(config, true).component2()
        )

        assertType<GetUsageError.NoAccount>(
            CoreModel.getLocalAndServerUsage(config, false).component2()
        )
    }

    @Test
    fun getLocalAndServerUsageUnexpectedError() {
        assertType<GetUsageError.Unexpected>(
            Klaxon().converter(getLocalAndServerUsageConverter).parse<Result<LocalAndServerUsages, GetUsageError>>(
                getLocalAndServerUsage("", false)
            )?.component2()
        )

        assertType<GetUsageError.Unexpected>(
            Klaxon().converter(getLocalAndServerUsageConverter).parse<Result<LocalAndServerUsages, GetUsageError>>(
                getLocalAndServerUsage("", true)
            )?.component2()
        )
    }
}
