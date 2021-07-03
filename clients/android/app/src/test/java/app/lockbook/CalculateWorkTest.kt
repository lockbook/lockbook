package app.lockbook

import app.lockbook.core.calculateWork
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class CalculateWorkTest {
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
    fun calculateWorkOk() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.calculateWork(config).unwrap()
    }

    @Test
    fun calculateWorkUnexpectedError() {
        Klaxon().converter(calculateWorkConverter)
            .parse<Result<WorkCalculated, CalculateWorkError>>(
                calculateWork("")
            ).unwrapErrorType<CalculateWorkError.Unexpected>()
    }
}
