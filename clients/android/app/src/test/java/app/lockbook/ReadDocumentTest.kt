package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.FileType
import app.lockbook.utils.ReadDocumentError
import com.beust.klaxon.Klaxon
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class ReadDocumentTest {

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
    fun readDocumentOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        coreModel.getDocumentContent(document.id).component1()!!
    }

    @Test
    fun readFolder() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val readDocumentError = coreModel.getDocumentContent(folder.id).component2()!!
        require(readDocumentError is ReadDocumentError.TreatedFolderAsDocument)
    }

    @Test
    fun readDocumentDoesNotExist() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val readDocumentError = coreModel.getDocumentContent(generateId()).component2()!!
        require(readDocumentError is ReadDocumentError.FileDoesNotExist)
    }
}