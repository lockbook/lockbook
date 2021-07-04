package app.lockbook

import app.lockbook.core.getUncompressedUsage
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<UsageItemMetric>(
            CoreModel.getUncompressedUsage(config).component1()
        )
    }

    @Test
    fun getUncompressedUsageUnexpectedError() {
        assertType<GetUsageError.Unexpected>(
            Klaxon().converter(getUncompressedUsageConverter).parse<Result<UsageItemMetric, GetUsageError>>(
                getUncompressedUsage("")
            )?.component2()
        )
    }
}
