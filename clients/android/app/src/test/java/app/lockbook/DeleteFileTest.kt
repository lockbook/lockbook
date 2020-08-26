package app.lockbook

import app.lockbook.core.deleteFile
import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class DeleteFileTest {

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
    fun deleteFileOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        coreModel.deleteFile(document.id).component1()!!
        coreModel.deleteFile(folder.id).component1()!!
    }

    @Test
    fun deleteFileNoFileWithThatId() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        coreModel.deleteFile(generateAlphaString()).component1()!!
    }

    @Test
    fun deleteFileUnexpectedError() {
        val deleteFile: Result<Unit, DeleteFileError>? =
            Klaxon().converter(deleteFileConverter).parse(deleteFile("", ""))
        val deleteFileError = deleteFile!!.component2()!!
        require(deleteFileError is DeleteFileError.UnexpectedError)
    }
}
