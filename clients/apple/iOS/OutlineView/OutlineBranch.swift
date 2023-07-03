///
/// Ripped from: https://github.com/toph-allen/OutlineView/blob/main/OutlineView/OutlineView.swift
///
import SwiftUI
import Combine
import SwiftLockbookCore
import UniformTypeIdentifiers

struct OutlineBranch: View {
    @EnvironmentObject var current: DocumentService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var status: StatusService
    @EnvironmentObject var errors: UnexpectedErrorService
    @EnvironmentObject var sheets: SheetState

    @StateObject var state: BranchState = BranchState()
    
    let file: File
    let level: CGFloat
    
    var children: [File] {
        files.childrenOf(file)
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    @ViewBuilder
    var body: some View {
        ScrollViewReader { scrollView in
            VStack(alignment: .leading) {
                if level != -1 {
                    if file == current.openDocuments.values.first?.meta {
                        OutlineRow(file: file, level: level, open: $state.open)
                            .background(Color.accentColor)
                            .foregroundColor(Color.white)
                            .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                    } else {
                        OutlineRow(file: file, level: level, open: $state.open)
                            .onTapGesture {
                                if file.fileType == .Folder {
                                    withAnimation {
                                        state.open.toggle()
                                    }
                                } else {
                                    // Animating this causes editor to load weirdly
                                    DispatchQueue.main.async {
                                        current.openDocuments.removeAll()
                                        current.openDoc(meta: file)
                                    }
                                    
                                    print("tap")
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
        }
    }
    
    func handleDelete(meta: File) -> () -> Void {
        {
            files.deleteFile(id: meta.id)
        }
    }
}
