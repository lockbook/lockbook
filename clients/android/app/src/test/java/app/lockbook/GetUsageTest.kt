package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.GetUsageError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetUsageTest {
    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, createRandomPath()))
    }

    @Test
    fun getUsageOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.getUsage().unwrapOk()
    }

    @Test
    fun getUsageNoAccount() {
        CoreModel.getUsage().unwrapErrorType(GetUsageError.NoAccount)
    }
}
