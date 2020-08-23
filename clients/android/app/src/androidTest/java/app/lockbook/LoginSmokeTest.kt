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

@RunWith(AndroidJUnit4::class)
class LoginSmokeTest {

    @get:Rule
    val activityRule = ActivityTestRule(InitialLaunchFigureOuter::class.java)

    private fun generateRandomUsername(): String =
        (1..10).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

    @Test
    fun testLogin() {
        onView(withId(R.id.welcome_new_lockbook)).perform(click())
        onView(withId(R.id.new_account_username)).perform(typeText(generateRandomUsername()))
        onView(withId(R.id.new_account_create_lockbook)).perform(click())
    }
}
