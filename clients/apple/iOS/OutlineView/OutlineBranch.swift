///
/// Ripped from: https://github.com/toph-allen/OutlineView/blob/main/OutlineView/OutlineView.swift
///
import SwiftUI
import Combine
import SwiftLockbookCore
import UniformTypeIdentifiers

struct OutlineBranch: View {
    @EnvironmentObject var files: FileService
    @EnvironmentObject var status: StatusService
    @EnvironmentObject var errors: UnexpectedErrorService
    
    @ObservedObject var outlineState: OutlineState
    @StateObject var state: BranchState = BranchState()
    
    let file: DecryptedFileMetadata
    let level: CGFloat
    
    var children: [DecryptedFileMetadata] {
        files.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    @ViewBuilder
    var body: some View {
        ScrollViewReader { scrollView in
            VStack(alignment: .leading) {
                if level != -1 {
                    if file == outlineState.selectedItem {
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
                                    outlineState.selectedItem = self.file
                                    print("tap")
                                }
                            }
                    }
                }
                
                if isLeaf == false && (state.open == true || level == -1) {
                    ForEach(children) { child in
                        OutlineBranch(outlineState: outlineState, file: child, level: self.level + 1)
                    }
                }
            }
            .contextMenu(menuItems: {
                OutlineContextMenu(meta: file, outlineState: outlineState, branchState: state)
            })
        }
    }
    
    func handleDelete(meta: DecryptedFileMetadata) -> () -> Void {
        return {
            files.deleteFile(id: meta.id)
        }
    }
}
