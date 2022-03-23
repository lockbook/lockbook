package app.lockbook

import app.lockbook.core.getUsage
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.GetUsageError
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.UsageMetrics
import kotlinx.serialization.decodeFromString
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
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getUsage(config).unwrapOk()
    }

    @Test
    fun getUsageNoAccount() {
        CoreModel.getUsage(config).unwrapErrorType(GetUsageError.NoAccount)
    }

    @Test
    fun getUsageUnexpectedError() {
        CoreModel.getUsageParser.decodeFromString<IntermCoreResult<UsageMetrics, GetUsageError>>(
            getUsage("")
        ).unwrapUnexpected()
    }
}
