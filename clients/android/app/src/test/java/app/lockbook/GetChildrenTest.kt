package app.lockbook

import app.lockbook.core.getChildren
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.DecryptedFileMetadata
import app.lockbook.util.GetChildrenError
import app.lockbook.util.IntermCoreResult
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetChildrenTest {
    var config = Config(createRandomPath())

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        config = Config(createRandomPath())
    }

    @Test
    fun getChildrenOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        CoreModel.getChildren(config, rootFileMetadata.id).unwrapOk()
    }

    @Test
    fun getChildrenUnexpectedError() {
        CoreModel.getChildrenParser.decodeFromString<IntermCoreResult<List<DecryptedFileMetadata>, GetChildrenError>>(
            getChildren("", "")
        ).unwrapUnexpected()
    }
}
