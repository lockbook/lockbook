package app.lockbook

import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.click
import androidx.test.espresso.action.ViewActions.typeText
import androidx.test.espresso.matcher.ViewMatchers.withId
import androidx.test.espresso.matcher.ViewMatchers.withText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.ActivityTestRule
import app.lockbook.loggedin.listfiles.ListFilesActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.util.*

@RunWith(AndroidJUnit4::class)
class NewFileSmokeTest {

    @get:Rule
    val activityRule = ActivityTestRule(ListFilesActivity::class.java)

    private fun generateRandomFileName(): String =
        (1..5).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

    @Test
    fun testNewDocumentCreation() {
        val documentName = generateRandomFileName()

        onView(withId(R.id.list_files_fab)).perform(click())
        onView(withId(R.id.list_files_fab_document)).perform(click())
        onView(withId(R.id.new_file_username)).perform(typeText(documentName))
        onView(withText("Create")).perform(click())
        onView(withText(documentName)).perform(click())
    }

    @Test
    fun testNewFolderCreation() {
        val folderName = generateRandomFileName()

        onView(withId(R.id.list_files_fab)).perform(click())
        onView(withId(R.id.list_files_fab_folder)).perform(click())
        onView(withId(R.id.new_file_username)).perform(typeText(folderName))
        onView(withText("Create")).perform(click())
        onView(withText(folderName)).perform(click())
    }
}
