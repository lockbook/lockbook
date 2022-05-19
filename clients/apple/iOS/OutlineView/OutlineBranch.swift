///
/// Ripped from: https://github.com/toph-allen/OutlineView/blob/main/OutlineView/OutlineView.swift
///
import SwiftUI
import Combine
import SwiftLockbookCore
import UniformTypeIdentifiers

struct OutlineBranch: View {
    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var files: FileService
    @EnvironmentObject var status: StatusService
    @EnvironmentObject var errors: UnexpectedErrorService
    @EnvironmentObject var sheets: SheetState

    @StateObject var state: BranchState = BranchState()
    
    let file: DecryptedFileMetadata
    let level: CGFloat
    
    var children: [DecryptedFileMetadata] {
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
                    if file == current.selectedItem {
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
                                    current.selectedItem = file
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
    
    func handleDelete(meta: DecryptedFileMetadata) -> () -> Void {
        {
            files.deleteFile(id: meta.id)
        }
    }
}
