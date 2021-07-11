package app.lockbook

import app.lockbook.core.getUncompressedUsage
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.getUncompressedUsage(config).unwrap()
    }

    @Test
    fun getUncompressedUsageUnexpectedError() {
        Klaxon().converter(getUncompressedUsageConverter).parse<Result<UsageItemMetric, GetUsageError>>(
            getUncompressedUsage("")
        ).unwrapErrorType<GetUsageError.Unexpected>()
    }
}
