package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.UpgradeAccountAndroid
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Ignore
import org.junit.Test

class UpgradeAccountAndroidTest {

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

    @Ignore("Does not work unless the environment variable GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY is defined in server.")
    @Test
    fun upgradeAccountAndroidInvalidPurchaseToken() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.upgradeAccountAndroid("", "").unwrapErrorType(UpgradeAccountAndroid.InvalidPurchaseToken)
    }
}
