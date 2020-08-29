package app.lockbook

import app.lockbook.core.calculateSyncWork
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
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun calculateWorkOk() {
        val coreModel = CoreModel(Config(path))
        val generateAccountResult = CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        )
        print("SMAILBARKOUCH123: ${Klaxon().toJsonString(generateAccountResult)}")
        generateAccountResult.component1()!!
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
