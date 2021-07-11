package app.lockbook

import app.lockbook.core.getChildren
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        CoreModel.getChildren(config, rootFileMetadata.id).unwrap()
    }

    @Test
    fun getChildrenUnexpectedError() {
        Klaxon().converter(getChildrenConverter)
            .parse<Result<List<ClientFileMetadata>, GetChildrenError>>(getChildren("", ""))
            .unwrapErrorType<GetChildrenError.Unexpected>()
    }
}
