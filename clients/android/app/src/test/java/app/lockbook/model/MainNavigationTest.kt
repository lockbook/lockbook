package app.lockbook.model

import androidx.lifecycle.SavedStateHandle
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MainNavigationTest {
    @Test
    fun searchPreservesFocusedDetail() {
        val focused =
            MainNavigationState(
                paneIntent = PaneIntent.FocusDetail,
            )

        val searching =
            reduceMainNavigation(
                focused,
                MainNavigationAction.OpenSearch(SearchPresentation.FullScreen),
            ).state
        assertEquals(SidebarDestination.Files, searching.sidebar)
        assertEquals(PaneIntent.FocusDetail, searching.paneIntent)
        assertEquals(
            SearchOverlay(
                originPaneIntent = PaneIntent.FocusDetail,
                presentation = SearchPresentation.FullScreen,
            ),
            searching.searchOverlay,
        )

        val restored = reduceMainNavigation(searching, MainNavigationAction.CloseSearch)
        assertEquals(SidebarDestination.Files, restored.state.sidebar)
        assertEquals(PaneIntent.FocusDetail, restored.state.paneIntent)
        assertEquals(null, restored.state.searchOverlay)
    }

    @Test
    fun selectingSearchResultOpensDocumentAndClearsSearch() {
        val searching =
            reduceMainNavigation(
                MainNavigationState(),
                MainNavigationAction.OpenSearch(SearchPresentation.SidebarMorph),
            ).state

        val transition =
            reduceMainNavigation(
                searching,
                MainNavigationAction.OpenDocument("document-id", newFile = false),
            )

        assertEquals(SidebarDestination.Files, transition.state.sidebar)
        assertEquals(PaneIntent.Detail, transition.state.paneIntent)
        assertEquals(null, transition.state.searchOverlay)
        assertEquals(
            MainNavigationEffect.OpenDocument("document-id", newFile = false),
            transition.effect,
        )
    }

    @Test
    fun backClosesDeclarativeDestinationsInPriorityOrder() {
        val state =
            MainNavigationState(
                sidebar = SidebarDestination.CreateLink("file-id"),
                searchOverlay =
                    SearchOverlay(
                        originPaneIntent = PaneIntent.Automatic,
                        presentation = SearchPresentation.SidebarMorph,
                    ),
            )

        val searchBack = reduceMainNavigation(state, MainNavigationAction.Back)
        assertTrue(searchBack.handled)
        assertEquals(null, searchBack.state.searchOverlay)
        assertTrue(searchBack.state.sidebar is SidebarDestination.CreateLink)

        val sidebarBack = reduceMainNavigation(searchBack.state, MainNavigationAction.Back)
        assertEquals(SidebarDestination.PendingShares, sidebarBack.state.sidebar)

        val unhandled = reduceMainNavigation(sidebarBack.state, MainNavigationAction.Back)
        assertFalse(unhandled.handled)
    }

    @Test
    fun folderSearchResultReturnsToSidebarFirstLayout() {
        val searching =
            reduceMainNavigation(
                MainNavigationState(
                    sidebar = SidebarDestination.Recents,
                    paneIntent = PaneIntent.Detail,
                ),
                MainNavigationAction.OpenSearch(SearchPresentation.FullScreen),
            ).state

        val result = reduceMainNavigation(searching, MainNavigationAction.OpenFolderFromSearch)

        assertEquals(SidebarDestination.Files, result.state.sidebar)
        assertEquals(PaneIntent.SplitOrSidebar, result.state.paneIntent)
        assertEquals(null, result.state.searchOverlay)
    }

    @Test
    fun durableSidebarDestinationsSurviveSavedStateRoundTrip() {
        val states =
            listOf(
                MainNavigationState(),
                MainNavigationState(sidebar = SidebarDestination.Recents),
                MainNavigationState(sidebar = SidebarDestination.PendingShares),
            )

        states.forEach { state ->
            val savedState = SavedStateHandle()
            with(MainScreenViewModel) {
                savedState.persistNavigationState(state)
                assertEquals(state, savedState.restoreNavigationState())
            }
        }
    }

    @Test
    fun transientNavigationStateRestoresOnlyItsDurableSidebar() {
        val states =
            mapOf(
                MainNavigationState(
                    sidebar = SidebarDestination.Recents,
                    paneIntent = PaneIntent.FocusDetail,
                    searchOverlay =
                        SearchOverlay(
                            originPaneIntent = PaneIntent.FocusDetail,
                            presentation = SearchPresentation.FullScreen,
                        ),
                ) to SidebarDestination.Recents,
                MainNavigationState(
                    sidebar = SidebarDestination.CreateLink("link-file"),
                    paneIntent = PaneIntent.Detail,
                ) to SidebarDestination.PendingShares,
            )

        states.forEach { (state, expectedSidebar) ->
            val savedState = SavedStateHandle()
            with(MainScreenViewModel) {
                savedState.persistNavigationState(state)
                assertEquals(MainNavigationState(sidebar = expectedSidebar), savedState.restoreNavigationState())
            }
        }
    }

    @Test
    fun persistedSidebarProvidesColdStartDefault() {
        val restored =
            with(MainScreenViewModel) {
                SavedStateHandle().restoreNavigationState(defaultRoot = SidebarRoot.Recents)
            }

        assertEquals(SidebarDestination.Recents, restored.sidebar)
    }

    @Test
    fun savedStateTakesPrecedenceOverColdStartDefault() {
        val savedState = SavedStateHandle()
        with(MainScreenViewModel) {
            savedState.persistNavigationState(MainNavigationState(sidebar = SidebarDestination.PendingShares))

            val restored = savedState.restoreNavigationState(defaultRoot = SidebarRoot.Recents)

            assertEquals(SidebarDestination.PendingShares, restored.sidebar)
        }
    }
}
