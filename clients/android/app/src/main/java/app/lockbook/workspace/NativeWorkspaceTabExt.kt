package app.lockbook.workspace

import app.lockbook.model.WorkspaceTab
import app.lockbook.model.WorkspaceTabType

fun NativeWorkspaceTab.toModelTab(): WorkspaceTab {
    if (this.type == WorkspaceTabType.Welcome.value || this.id.isNullUUID()) {
        return WorkspaceTab.welcome
    }

    val tabType = WorkspaceTabType.fromInt(this.type) ?: WorkspaceTabType.Welcome
    return WorkspaceTab(this.id, tabType)
}
