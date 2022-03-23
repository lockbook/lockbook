package app.lockbook

import app.lockbook.core.getUncompressedUsage
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.GetUsageError
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.UsageItemMetric
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetUncompressedUsageTest {
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
    fun getUncompressedUsageOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getUncompressedUsage(config).unwrapOk()
    }

    @Test
    fun getUncompressedUsageUnexpectedError() {
        CoreModel.getUncompressedUsageParser.decodeFromString<IntermCoreResult<UsageItemMetric, GetUsageError>>(
            getUncompressedUsage("")
        ).unwrapUnexpected()
    }
}
