package app.lockbook

import app.lockbook.core.getRoot
import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetRootTest {

    private val coreModel = CoreModel(Config(path))

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
            Runtime.getRuntime().exec("rm -rf $path")
        }
    }

    @Before
    fun createDirectory() {
        Runtime.getRuntime().exec("mkdir $path")
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path")
    }

    @Test
    fun getRootOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
    }

    @Test
    fun getRootUnexpectedError() {
        val getRootResult: Result<FileMetadata, GetRootError>? =
            Klaxon().converter(getRootConverter).parse(getRoot(""))
        val getRootError = getRootResult!!.component2()!!
        require(getRootError is GetRootError.UnexpectedError)
    }
}
