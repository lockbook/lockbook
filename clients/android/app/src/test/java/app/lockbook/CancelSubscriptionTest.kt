package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.CancelSubscriptionError
import app.lockbook.util.Config
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class CancelSubscriptionTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lb_c_v1")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, false, createRandomPath()))
    }

    @Test
    fun cancelSubscriptionNotPremium() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.cancelSubscription().unwrapErrorType(CancelSubscriptionError.NotPremium)
    }
}
