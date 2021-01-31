package app.lockbook

import app.lockbook.core.calculateWork
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )
        assertType<WorkCalculated>(
            CoreModel.calculateWork(config).component1()
        )
    }

    @Test
    fun calculateWorkUnexpectedError() {
        val calculateSyncWorkResult: Result<WorkCalculated, CalculateWorkError>? =
            Klaxon().converter(calculateWorkConverter).parse(calculateWork(""))

        assertType<CalculateWorkError.Unexpected>(
            calculateSyncWorkResult?.component2()
        )
    }
}
