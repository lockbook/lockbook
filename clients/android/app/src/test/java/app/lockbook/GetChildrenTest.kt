package app.lockbook

import app.lockbook.core.getChildren
import app.lockbook.core.getFileById
import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class GetChildrenTest {

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
    fun getChildrenOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        coreModel.getChildrenOfParent().component1()!!
        coreModel.getParentOfParent().component1()!!
    }

    @Test
    fun getChildrenUnexpectedError() {
        val getChildrenResult: Result<List<FileMetadata>, GetChildrenError>? =
            Klaxon().converter(getChildrenConverter)
                .parse(getChildren("", ""))
        val getChildrenError = getChildrenResult!!.component2()!!
        require(getChildrenError is GetChildrenError.UnexpectedError)
    }
}
