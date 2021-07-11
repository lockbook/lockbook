package app.lockbook

import app.lockbook.core.getUsage
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
    fun getUsageOk() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.getUsage(config).unwrap()
    }

    @Test
    fun getUsageNoAccount() {
        CoreModel.getUsage(config).unwrapErrorType<GetUsageError.NoAccount>()
    }

    @Test
    fun getUsageUnexpectedError() {
        Klaxon().converter(getUsageConverter)
            .parse<Result<UsageMetrics, GetUsageError>>(getUsage(""))
            .unwrapErrorType<GetUsageError.Unexpected>()
    }
}
