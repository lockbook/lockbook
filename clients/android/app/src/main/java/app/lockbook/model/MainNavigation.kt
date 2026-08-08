package app.lockbook.model

enum class PaneIntent {
    Automatic,
    Sidebar,
    Detail,
    FocusDetail,
    SplitOrSidebar,
}

enum class SidebarRoot {
    Files,
    Recents,
    PendingShares,
}

sealed interface SidebarDestination {
    data object Files : SidebarDestination

    data object Recents : SidebarDestination

    data object PendingShares : SidebarDestination

    data class CreateLink(
        val fileId: String,
    ) : SidebarDestination
}

data class SearchOverlay(
    val openedFromWorkspace: Boolean,
    val originPaneIntent: PaneIntent,
)

data class MainNavigationState(
    val sidebar: SidebarDestination = SidebarDestination.Files,
    val paneIntent: PaneIntent = PaneIntent.Automatic,
    val searchOverlay: SearchOverlay? = null,
)

sealed interface MainNavigationAction {
    data class SelectSidebar(
        val destination: SidebarRoot,
    ) : MainNavigationAction

    data class OpenSearch(
        val fromWorkspace: Boolean,
    ) : MainNavigationAction

    data object CloseSearch : MainNavigationAction

    data object OpenFolderFromSearch : MainNavigationAction

    data class OpenDocument(
        val id: String,
        val newFile: Boolean,
    ) : MainNavigationAction

    data class CreateLink(
        val fileId: String,
    ) : MainNavigationAction

    data object ShowSidebar : MainNavigationAction

    data object ShowDetail : MainNavigationAction

    data object FocusDetail : MainNavigationAction

    data object ShowSplitOrSidebar : MainNavigationAction

    data object Back : MainNavigationAction
}

sealed interface MainNavigationEffect {
    data class OpenDocument(
        val id: String,
        val newFile: Boolean,
    ) : MainNavigationEffect
}

data class MainNavigationTransition(
    val state: MainNavigationState,
    val effect: MainNavigationEffect? = null,
    val handled: Boolean = true,
)

fun reduceMainNavigation(
    state: MainNavigationState,
    action: MainNavigationAction,
): MainNavigationTransition {
    return when (action) {
        is MainNavigationAction.SelectSidebar -> {
            MainNavigationTransition(
                state.copy(
                    sidebar = action.destination.asDestination(),
                    searchOverlay = null,
                ),
            )
        }

        is MainNavigationAction.OpenSearch -> {
            MainNavigationTransition(
                state.copy(
                    searchOverlay =
                        SearchOverlay(
                            openedFromWorkspace = action.fromWorkspace,
                            originPaneIntent = state.paneIntent,
                        ),
                ),
            )
        }

        MainNavigationAction.CloseSearch -> {
            if (state.searchOverlay == null) return MainNavigationTransition(state, handled = false)
            MainNavigationTransition(state.copy(searchOverlay = null))
        }

        MainNavigationAction.OpenFolderFromSearch -> {
            if (state.searchOverlay == null) return MainNavigationTransition(state, handled = false)
            MainNavigationTransition(
                state.copy(
                    sidebar = SidebarDestination.Files,
                    paneIntent = PaneIntent.SplitOrSidebar,
                    searchOverlay = null,
                ),
            )
        }

        is MainNavigationAction.OpenDocument -> {
            val search = state.searchOverlay
            val paneIntent =
                if (search?.originPaneIntent == PaneIntent.FocusDetail) {
                    PaneIntent.FocusDetail
                } else {
                    PaneIntent.Detail
                }
            MainNavigationTransition(
                state =
                    state.copy(
                        paneIntent = paneIntent,
                        searchOverlay = null,
                    ),
                effect = MainNavigationEffect.OpenDocument(action.id, action.newFile),
            )
        }

        is MainNavigationAction.CreateLink -> {
            MainNavigationTransition(
                state.copy(sidebar = SidebarDestination.CreateLink(action.fileId)),
            )
        }

        MainNavigationAction.ShowSidebar -> {
            MainNavigationTransition(state.copy(paneIntent = PaneIntent.Sidebar))
        }

        MainNavigationAction.ShowDetail -> {
            MainNavigationTransition(state.copy(paneIntent = PaneIntent.Detail))
        }

        MainNavigationAction.FocusDetail -> {
            MainNavigationTransition(state.copy(paneIntent = PaneIntent.FocusDetail))
        }

        MainNavigationAction.ShowSplitOrSidebar -> {
            MainNavigationTransition(state.copy(paneIntent = PaneIntent.SplitOrSidebar))
        }

        MainNavigationAction.Back -> {
            when {
                state.searchOverlay != null -> {
                    reduceMainNavigation(state, MainNavigationAction.CloseSearch)
                }

                state.sidebar is SidebarDestination.CreateLink -> {
                    MainNavigationTransition(state.copy(sidebar = SidebarDestination.PendingShares))
                }

                else -> {
                    MainNavigationTransition(state, handled = false)
                }
            }
        }
    }
}

internal fun SidebarRoot.asDestination(): SidebarDestination =
    when (this) {
        SidebarRoot.Files -> SidebarDestination.Files
        SidebarRoot.Recents -> SidebarDestination.Recents
        SidebarRoot.PendingShares -> SidebarDestination.PendingShares
    }
