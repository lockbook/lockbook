#if DEBUG
    import SwiftUI
    import SwiftWorkspace

    extension View {
        func withCommonPreviewEnvironment() -> some View {
            var preview =
                self
                // !!!: ADD NEW OBSERVABLE OBJECTS TO THIS LIST
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

            #if os(iOS)
                preview = preview.environmentObject(FileTreeViewModel.preview)
            #else
                // do nothing... for now
            #endif

            return preview
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
