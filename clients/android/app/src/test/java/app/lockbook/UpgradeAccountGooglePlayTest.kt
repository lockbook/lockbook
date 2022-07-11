package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.UpgradeAccountGooglePlayError
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Ignore
import org.junit.Test

class UpgradeAccountGooglePlayTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @Before
    fun initCore() {
        CoreModel.init(Config(false, false, createRandomPath()))
    }

    @Ignore("Does not work unless the environment variable GOOGLE_CLOUD_SERVICE_ACCOUNT_KEY is defined in server.")
    @Test
    fun upgradeAccountGooglePlayInvalidPurchaseToken() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        CoreModel.upgradeAccountGooglePlay("", "").unwrapErrorType(UpgradeAccountGooglePlayError.InvalidPurchaseToken)
    }
}
