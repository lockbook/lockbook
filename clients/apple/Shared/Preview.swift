#if DEBUG
    import SwiftUI
    import SwiftWorkspace

    // !!!: ADD NEW OBSERVABLE OBJECTS TO THIS
    extension View {
        func withCommonPreviewEnvironment() -> some View {
            self
                .environmentObject(BillingState.preview)
                .environmentObject(FilesViewModel.preview)
                .environmentObject(HomeState.preview)
                .environmentObject(PathSearchViewModel.preview)
                .environmentObject(PendingSharesViewModel.preview)
                .environmentObject(SelectFolderViewModel.preview)
                .environmentObject(SettingsViewModel.preview)
                .environmentObject(SuggestedDocsViewModel.preview)
                .environmentObject(WorkspaceInputState.preview)
                .environmentObject(WorkspaceOutputState.preview)
                .withPlatformSpecificPreviewEnvironment()
        }

        private func withPlatformSpecificPreviewEnvironment() -> some View {
            #if os(iOS)
                return
                    self
                    .environmentObject(FileTreeViewModel.preview)
            #else
                return self
            #endif
        }
    }

    extension View {
        func withMacPreviewSize(width: CGFloat = 400, height: CGFloat = 80)
            -> some View
        {
            #if os(macOS)
                self.frame(width: width, height: height)
            #else
                self
            #endif
        }
    }
#endif
