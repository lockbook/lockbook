package app.lockbook

import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.action.ViewActions.click
import androidx.test.espresso.action.ViewActions.typeText
import androidx.test.espresso.matcher.ViewMatchers.withId
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.rule.ActivityTestRule
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.util.*

@RunWith(AndroidJUnit4::class)
class ImportSmokeTest {

    @get:Rule
    val activityRule = ActivityTestRule(InitialLaunchFigureOuter::class.java)

    private fun generateUuid(): String = UUID.randomUUID().toString()

    @Test
    fun testImport() {
        onView(withId(R.id.welcome_import_lockbook)).perform(click())
        onView(withId(R.id.text_import_account_string))
            .perform(typeText(generateUuid()))
        onView(withId(R.id.import_lockbook))
            .perform(click())
    }
}
