@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.model

import android.app.Application
import android.content.Context
import androidx.core.content.edit
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.viewModelScope
import app.lockbook.util.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError
import java.io.File

class MainScreenViewModel(
    application: Application,
    private val savedStateHandle: SavedStateHandle,
) : AndroidViewModel(application) {
    var activityScreen: ActivityScreen? = null
    private val _launchActivityScreen = SingleMutableLiveData<ActivityScreen>()
    private val _launchTransientScreen = SingleMutableLiveData<TransientScreen>()
    private val _mainUiEffect = SingleMutableLiveData<MainUiEffect>()
    private val navigationPreferences =
        application.getSharedPreferences(MAIN_NAVIGATION_PREFERENCES, Context.MODE_PRIVATE)
    private val _navigationState =
        MutableStateFlow(
            savedStateHandle.restoreNavigationState(
                defaultRoot = navigationPreferences.getLastSidebarRoot(),
            ),
        )
    private val _navigationEffects = Channel<MainNavigationEffect>(Channel.BUFFERED)

    val launchActivityScreen: LiveData<ActivityScreen?>
        get() = _launchActivityScreen

    val launchTransientScreen: LiveData<TransientScreen>
        get() = _launchTransientScreen

    val mainUiEffect: LiveData<MainUiEffect>
        get() = _mainUiEffect

    val navigationState: StateFlow<MainNavigationState> = _navigationState.asStateFlow()
    val navigationEffects = _navigationEffects.receiveAsFlow()

    val exportImportModel = ExportImportModel(_mainUiEffect)

    fun launchActivityScreen(screen: ActivityScreen) {
        activityScreen = screen
        _launchActivityScreen.postValue(activityScreen)
    }

    fun launchTransientScreen(screen: TransientScreen) {
        _launchTransientScreen.postValue(screen)
    }

    fun navigate(action: MainNavigationAction): Boolean {
        val transition = reduceMainNavigation(_navigationState.value, action)
        if (transition.state != _navigationState.value) {
            _navigationState.value = transition.state
            savedStateHandle.persistNavigationState(transition.state)
            navigationPreferences.edit {
                putString(LAST_SIDEBAR_DESTINATION_KEY, transition.state.sidebar.persistedRootDestination())
            }
        }
        transition.effect?.let { _navigationEffects.trySend(it) }
        return transition.handled
    }

    fun showProgressOverlay(show: Boolean) {
        _mainUiEffect.value = MainUiEffect.ShowHideProgressOverlay(show)
    }

    fun shareSelectedFiles(
        selectedFiles: List<net.lockbook.File>,
        appDataDir: File,
    ) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                exportImportModel.exportDocuments(selectedFiles, appDataDir)
            } catch (err: LbError) {
                _mainUiEffect.postValue(MainUiEffect.NotifyError(err))
            }
        }
    }

    fun confirmSubscription(
        purchaseToken: String,
        accountId: String,
    ) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                Lb.upgradeAccountGooglePlay(purchaseToken, accountId)
                _mainUiEffect.postValue(MainUiEffect.ShowSubscriptionConfirmed)
            } catch (err: LbError) {
                _mainUiEffect.postValue(
                    MainUiEffect.NotifyError(err),
                )
            }
        }
    }

    companion object {
        private const val MAIN_NAVIGATION_PREFERENCES = "main_navigation_preferences"
        private const val LAST_SIDEBAR_DESTINATION_KEY = "last_sidebar_destination"
        private const val DURABLE_SIDEBAR_DESTINATION_KEY = "main_navigation_sidebar"

        private const val FILES_DESTINATION = "files"
        private const val RECENTS_DESTINATION = "recents"
        private const val PENDING_SHARES_DESTINATION = "pending_shares"

        internal fun SavedStateHandle.restoreNavigationState(defaultRoot: SidebarRoot = SidebarRoot.Files): MainNavigationState =
            MainNavigationState(
                sidebar =
                    when (get<String>(DURABLE_SIDEBAR_DESTINATION_KEY)) {
                        FILES_DESTINATION -> SidebarDestination.Files
                        RECENTS_DESTINATION -> SidebarDestination.Recents
                        PENDING_SHARES_DESTINATION -> SidebarDestination.PendingShares
                        else -> defaultRoot.asDestination()
                    },
            )

        private fun android.content.SharedPreferences.getLastSidebarRoot(): SidebarRoot =
            when (getString(LAST_SIDEBAR_DESTINATION_KEY, FILES_DESTINATION)) {
                RECENTS_DESTINATION -> SidebarRoot.Recents
                PENDING_SHARES_DESTINATION -> SidebarRoot.PendingShares
                else -> SidebarRoot.Files
            }

        private fun SidebarDestination.persistedRootDestination(): String =
            when (this) {
                SidebarDestination.Files -> {
                    FILES_DESTINATION
                }

                SidebarDestination.Recents -> {
                    RECENTS_DESTINATION
                }

                SidebarDestination.PendingShares -> {
                    PENDING_SHARES_DESTINATION
                }

                is SidebarDestination.CreateLink -> {
                    PENDING_SHARES_DESTINATION
                }
            }

        internal fun SavedStateHandle.persistNavigationState(state: MainNavigationState) {
            this[DURABLE_SIDEBAR_DESTINATION_KEY] = state.sidebar.persistedRootDestination()
        }
    }
}

sealed class ActivityScreen {
    data class Settings(
        val scrollToPreference: Int? = null,
    ) : ActivityScreen()
}

sealed class TransientScreen {
    data class Move(
        val files: List<net.lockbook.File>,
    ) : TransientScreen()

    data class Rename(
        val file: net.lockbook.File,
    ) : TransientScreen()

    data class Create(
        val parentId: String,
    ) : TransientScreen()

    data class Info(
        val file: net.lockbook.File,
    ) : TransientScreen()

    data class Share(
        val file: net.lockbook.File,
    ) : TransientScreen()

    data class ShareExport(
        val files: List<File>,
    ) : TransientScreen()

    data class Delete(
        val files: List<net.lockbook.File>,
    ) : TransientScreen()
}

sealed class MainUiEffect {
    data class ShowHideProgressOverlay(
        val show: Boolean,
    ) : MainUiEffect()

    data class ShareDocuments(
        val files: ArrayList<File>,
    ) : MainUiEffect()

    data class NotifyError(
        val error: LbError,
    ) : MainUiEffect()

    object ShowSubscriptionConfirmed : MainUiEffect()
}
