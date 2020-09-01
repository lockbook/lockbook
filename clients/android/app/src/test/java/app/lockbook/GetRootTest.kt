package app.lockbook

import app.lockbook.core.getRoot
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetRootTest {
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
    fun getRootOk() {
        assertType<Unit>(
            this::getRootOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<FileMetadata>(
            this::getRootOk.name,
            CoreModel.getRoot(config).component1()
        )
    }

    @Test
    fun getRootUnexpectedError() {
        val getRootResult: Result<FileMetadata, GetRootError>? =
            Klaxon().converter(getRootConverter).parse(getRoot(""))

        assertType<GetRootError.UnexpectedError>(
            this::getRootUnexpectedError.name,
            getRootResult?.component2()
        )
    }
}
