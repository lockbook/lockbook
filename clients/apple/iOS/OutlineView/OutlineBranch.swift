///
/// Ripped from: https://github.com/toph-allen/OutlineView/blob/main/OutlineView/OutlineView.swift
///
import SwiftUI
import Combine
import UniformTypeIdentifiers
import SwiftWorkspace

struct OutlineBranch: View {
    @EnvironmentObject var files: FileService
    @EnvironmentObject var selected: SelectedFilesState
    @EnvironmentObject var errors: UnexpectedErrorService
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var workspace: WorkspaceState

    @StateObject var state: BranchState = BranchState()
    
    let file: File
    let level: CGFloat
    
    var children: [File] {
        files.childrenOf(file)
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    var isSelected: Bool {
        selected.totalSelectedFiles?.contains(file) == true
    }
    
    var isSelectable: Bool {
        selected.selectedFiles != nil
    }
    
    @ViewBuilder
    var body: some View {
        ScrollViewReader { scrollView in
            VStack(alignment: .leading, spacing: 0) {
                if level != -1 {
                    OutlineRow(file: file, level: level, open: $state.open)
                        .onTapGesture {
                            if isSelectable {
                                if isSelected {
                                    selected.removeFileFromSelection(file: file)
                                } else {
                                    selected.addFileToSelection(file: file)
                                }
                            } else {
                                if file.type == .folder {
                                    workspace.selectedFolder = file.id
                                    
                                    withAnimation {
                                        state.open.toggle()
                                    }
                                } else {
                                    DI.workspace.requestOpenDoc(file.id)
                                }
                            }
                        }
                }
                
                if isLeaf == false && (state.open == true || level == -1) {
                    ForEach(children) { child in
                        OutlineBranch(file: child, level: self.level + 1)
                    }
                }
            }
            .contextMenu(menuItems: {
                OutlineContextMenu(meta: file, branchState: state)
            })
            .confirmationDialog("Are you sure? This action cannot be undone.", isPresented: Binding(get: { sheets.deleteConfirmationInfo?.count == 1 && sheets.deleteConfirmationInfo?[0].id == file.id }, set: { sheets.deleteConfirmation = $0 }), titleVisibility: .visible, actions: {
                if let metas = sheets.deleteConfirmationInfo {
                    DeleteConfirmationButtons(metas: metas)
                }
            })
        }
    }
}
