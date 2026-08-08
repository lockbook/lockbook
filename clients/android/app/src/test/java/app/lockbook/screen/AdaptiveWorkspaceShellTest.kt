package app.lockbook.screen

import org.junit.Assert.assertEquals
import org.junit.Test

class AdaptiveWorkspaceShellTest {
    @Test
    fun autoModeTracksAvailableWidth() {
        assertEquals(
            WorkspaceShellMode.SidebarOnly,
            resolveWorkspaceShellMode(WorkspaceShellRequest.Auto, widthDp = 699f),
        )
        assertEquals(
            WorkspaceShellMode.Split,
            resolveWorkspaceShellMode(WorkspaceShellRequest.Auto, widthDp = 700f),
        )
    }

    @Test
    fun detailVisibleRestoresSplitWhenWidthGrows() {
        assertEquals(
            WorkspaceShellMode.DetailOnly,
            resolveWorkspaceShellMode(WorkspaceShellRequest.DetailVisible, widthDp = 600f),
        )
        assertEquals(
            WorkspaceShellMode.Split,
            resolveWorkspaceShellMode(WorkspaceShellRequest.DetailVisible, widthDp = 900f),
        )
    }

    @Test
    fun splitOrSidebarRestoresSplitWhenWidthGrows() {
        assertEquals(
            WorkspaceShellMode.SidebarOnly,
            resolveWorkspaceShellMode(WorkspaceShellRequest.SplitOrSidebar, widthDp = 600f),
        )
        assertEquals(
            WorkspaceShellMode.Split,
            resolveWorkspaceShellMode(WorkspaceShellRequest.SplitOrSidebar, widthDp = 900f),
        )
    }

    @Test
    fun focusedDetailRemainsFocusedAtEveryWidth() {
        assertEquals(
            WorkspaceShellMode.DetailOnly,
            resolveWorkspaceShellMode(WorkspaceShellRequest.DetailOnly, widthDp = 600f),
        )
        assertEquals(
            WorkspaceShellMode.DetailOnly,
            resolveWorkspaceShellMode(WorkspaceShellRequest.DetailOnly, widthDp = 900f),
        )
    }
}
