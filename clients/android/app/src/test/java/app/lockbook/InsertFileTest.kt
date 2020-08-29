package app.lockbook

import app.lockbook.core.insertFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class InsertFileTest {
    var path = createRandomPath()

    private val coreModel = CoreModel(Config(path))
    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun insertFileOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
    }

    @Test
    fun insertFileError() {
        val insertResult: Result<Unit, InsertFileError>? =
            Klaxon().converter(insertFileConverter)
                .parse(insertFile("", ""))
        val insertError = insertResult!!.component2()!!
        require(insertError is InsertFileError.UnexpectedError)
    }
}
