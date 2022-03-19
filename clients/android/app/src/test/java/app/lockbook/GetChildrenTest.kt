package app.lockbook

import app.lockbook.core.getAccount
import app.lockbook.core.getChildren
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        CoreModel.getChildren(config, rootFileMetadata.id).unwrap()
    }

    @Test
    fun getChildrenUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<List<DecryptedFileMetadata>, GetChildrenError>>(
            getChildren("", "")
        ).unwrapUnexpected()
    }
}
