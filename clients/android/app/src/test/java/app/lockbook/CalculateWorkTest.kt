package app.lockbook

import app.lockbook.core.calculateSyncWork
import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class CalculateWorkTest {
    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun calculateWorkOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.calculateFileSyncWork().component1()!!
    }

    @Test
    fun calculateWorkNoAccount() {
        val calculateSyncWorkResult: Result<WorkCalculated, CalculateWorkError>? =
            Klaxon().converter(calculateSyncWorkConverter).parse(calculateSyncWork(""))
        val calculateWorkError = calculateSyncWorkResult!!.component2()!!
        require(calculateWorkError is CalculateWorkError.UnexpectedError)
    }
}
