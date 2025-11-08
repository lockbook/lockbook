package app.lockbook

import app.lockbook.util.Config
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetUsageTest {
    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lb_external_interface")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, false, createRandomPath()))
    }

    @Test
    fun getUsageOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.getUsage().unwrapOk()
    }
}
