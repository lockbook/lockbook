package app.lockbook.screen

import android.os.Bundle
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.model.DetailsScreen
import app.lockbook.model.TransientScreen
import app.lockbook.model.StateViewModel

class MainScreenActivity: AppCompatActivity() {

    private val model: StateViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        model.launchDetailsScreen.observe(
            this,
            { screen ->
                launchDetailsScreen(screen)
            }
        )

        model.launchTransientScreen.observe(
            this,
            { screen ->
                launchDialogScreen(screen)
            }
        )
    }

    private fun launchDialogScreen(screen: TransientScreen) {
        when(screen) {
            is TransientScreen.Move -> TODO()
            is TransientScreen.Rename -> TODO()
            is TransientScreen.Create -> TODO()
            is TransientScreen.Info -> TODO()
            is TransientScreen.Share -> TODO()
        }
    }

    private fun launchDetailsScreen(screen: DetailsScreen) {
        when(screen) {
            DetailsScreen.Blank -> {}
            DetailsScreen.TextEditor -> {}
            DetailsScreen.Drawing -> TODO()
        }
    }
}