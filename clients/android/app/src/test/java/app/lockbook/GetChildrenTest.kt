package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetChildrenTest {

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
    fun getChildrenOk() {
        CoreModel.createAccount(generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot().unwrapOk()

        CoreModel.getChildren(rootFileMetadata.id).unwrapOk()
    }
}
