package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.CalculateWorkError
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.FileType
import com.beust.klaxon.Klaxon
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class CalculateWorkTest {
    private val coreModel = CoreModel(Config(path))

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
            Runtime.getRuntime().exec("rm -rf $path")
        }
    }

    @Before
    fun createDirectory() {
        Runtime.getRuntime().exec("mkdir $path")
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path")
    }

    @Test
    fun calculateWorkOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.calculateFileSyncWork().component1()!!
    }

    @Test
    fun calculateWorkNoAccount() {
        val calculateWorkError = coreModel.calculateFileSyncWork().component2()!!
        require(calculateWorkError is CalculateWorkError.NoAccount)
    }
}