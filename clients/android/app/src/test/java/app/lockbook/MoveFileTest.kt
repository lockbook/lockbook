package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.core.moveFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test
import java.util.*

class MoveFileTest {
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
    fun moveFileOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        coreModel.moveFile(document.id, folder.id).component1()!!
    }

    @Test
    fun moveFileDoesNotExist() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val moveFileError = coreModel.moveFile(generateId(), folder.id).component2()!!
        require(moveFileError is MoveFileError.FileDoesNotExist)
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val moveFileError = coreModel.moveFile(folder.id, document.id).component2()!!
        require(moveFileError is MoveFileError.DocumentTreatedAsFolder)
    }

    @Test
    fun moveFileTargetParentDoesNotExist() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val moveFileError = coreModel.moveFile(document.id, generateId()).component2()!!
        require(moveFileError is MoveFileError.TargetParentDoesNotExist)
    }

    @Test
    fun moveFIleTargetParentHasChildNamedThat() {
        val documentName = generateAlphaString()
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        coreModel.parentFileMetadata = folder
        val firstDocument = coreModel.createFile(documentName, Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(firstDocument).component1()!!
        coreModel.setParentToRoot()
        val secondDocument = coreModel.createFile(documentName, Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(secondDocument).component1()!!
        val moveFileError = coreModel.moveFile(secondDocument.id, folder.id).component2()!!
        require(moveFileError is MoveFileError.TargetParentHasChildNamedThat)
    }

    @Test
    fun moveFileUnexpectedError() {
        val moveResult: Result<Unit, MoveFileError>? =
            Klaxon().converter(moveFileConverter).parse(moveFile("", "", ""))
        val moveError = moveResult!!.component2()!!
        require(moveError is MoveFileError.UnexpectedError)
    }
}
