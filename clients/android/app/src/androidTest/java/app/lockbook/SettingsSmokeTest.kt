package app.lockbook

import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.click
import androidx.test.espresso.matcher.ViewMatchers.withId
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.ActivityTestRule
import app.lockbook.loggedin.listfiles.ListFilesActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SettingsSmokeTest {

    @get:Rule
    val activityRule = ActivityTestRule(ListFilesActivity::class.java)

    @Test
    fun openSettings() {
        onView(withId(R.id.menu_list_files_settings)).perform(click())
    }
}
