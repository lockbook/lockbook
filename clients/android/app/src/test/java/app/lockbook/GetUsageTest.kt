package app.lockbook

import app.lockbook.core.getLocalAndServerUsage
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.getLocalAndServerUsage(config, true).unwrap()

        CoreModel.getLocalAndServerUsage(config, false).unwrap()
    }

    @Test
    fun getLocalAndServerUsageNoAccount() {
        CoreModel.getLocalAndServerUsage(config, true).unwrapErrorType<GetUsageError.NoAccount>()

        CoreModel.getLocalAndServerUsage(config, false).unwrapErrorType<GetUsageError.NoAccount>()
    }

    @Test
    fun getLocalAndServerUsageUnexpectedError() {
        Klaxon().converter(getLocalAndServerUsageConverter)
            .parse<Result<LocalAndServerUsages, GetUsageError>>(
                getLocalAndServerUsage("", false)
            ).unwrapErrorType<GetUsageError.Unexpected>()

        Klaxon().converter(getLocalAndServerUsageConverter)
            .parse<Result<LocalAndServerUsages, GetUsageError>>(
                getLocalAndServerUsage("", true)
            ).unwrapErrorType<GetUsageError.Unexpected>()
    }
}
