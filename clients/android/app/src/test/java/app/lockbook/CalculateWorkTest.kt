package app.lockbook

import app.lockbook.core.calculateSyncWork
import app.lockbook.utils.*
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
            CoreModel.calculateFileSyncWork(config).component1()
        )
    }

    @Test
    fun calculateWorkUnexpectedError() {
        val calculateSyncWorkResult: Result<WorkCalculated, CalculateWorkError>? =
            Klaxon().converter(calculateSyncWorkConverter).parse(calculateSyncWork(""))

        assertType<CalculateWorkError.Unexpected>(
            calculateSyncWorkResult?.component2()
        )
    }
}
