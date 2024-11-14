package app.lockbook

import app.lockbook.util.Config
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetSubscriptionInfoTest {

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
    fun getSubscriptionInfoOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        assert(CoreModel.getSubscriptionInfo().unwrapOk() == null)
    }
}
