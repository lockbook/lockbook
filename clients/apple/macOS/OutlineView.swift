//
//  OutlineView.swift
//  OutlineView
//
//  Created by Toph Allen on 4/13/20.
//  Copyright © 2020 Toph Allen. All rights reserved.
//
import Foundation
import SwiftUI
import Combine
import SwiftLockbookCore

// This view handles displaying the contents of each row for its object. Clicking its arrow image also toggles a node's open state.
struct OutlineRow: View {
    @ObservedObject var core: Core
    var file: FileMetadata
    var level: CGFloat
    @Binding var open: Bool
    
    var children: [FileMetadata] {
        core.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    var body: some View {
        HStack {
            Group {
                if !isLeaf {
                    Image(open == false ? "arrowtriangle.right.fill.13-regular-small" : "arrowtriangle.down.fill.13-regular-small")
                        .renderingMode(.template)
                        .foregroundColor(Color.secondary)
                } else {
                    Image("arrowtriangle.right.fill.13-regular-small")
                        .opacity(0)
                }
            }
            .frame(width: 16, height: 16)
            .onTapGesture {
                open.toggle()
            }
            
            Image(file.fileType == .Folder ? "folder.13-regular-medium" : "doc.13-regular-medium")
                .renderingMode(.template)
                .frame(width: 16, height: 16)
                .padding(.leading, -4)
            
            Text(file.name)
                .lineLimit(1) // If lineLimit is not specified, non-leaf names will wrap
                .truncationMode(.tail)
                .allowsTightening(true)
            
            Spacer()
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .padding(.leading, level * 20)
    }
}


struct OutlineBranch: View {
    @ObservedObject var core: Core
    
    var file: FileMetadata
    @Binding var selectedItem: FileMetadata?
    var level: CGFloat
    @State var open: Bool = false
    
    var children: [FileMetadata] {
        core.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    @ViewBuilder
    var body: some View {
        VStack(spacing: 2) { // spacing: 2 is what List uses
            if level == -1 {
                EmptyView() // the root node is at
            } else {
                
                if file == selectedItem {
                    OutlineRow(core: core, file: file, level: level, open: $open)
                        .background(Color.accentColor)
                        .foregroundColor(Color.white)
                        .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                } else {
                    OutlineRow(core: core, file: file, level: level, open: $open)
                        .onTapGesture {
                            if file.fileType == .Folder {
                                self.open.toggle()
                            } else {
                                self.selectedItem = self.file
                            }
                        }
                }
            }
            if isLeaf == false && (open == true || level == -1) {
                ForEach(children) { child in
                    OutlineBranch(core: core, file: child, selectedItem: self.$selectedItem, level: self.level + 1)
                }
            }
        }
    }
}


struct OutlineSection: View {
    
    @ObservedObject var core: Core
    
    var root: FileMetadata
    @Binding var selectedItem: FileMetadata?
    
    var children: [FileMetadata] {
        core.files.filter {
            $0.parent == root.id && $0.id != root.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    var body: some View {
        List {
            // The padding in the section header is there to adjust for the inset hack.
            Section(header: Text(root.name).padding(.leading, 8)) {
                OutlineBranch(core: core, file: root, selectedItem: self.$selectedItem, level: -1)
            }
            .collapsible(false)
        }
        .listStyle(SidebarListStyle())
        .frame(minWidth: 10, maxWidth: .infinity, maxHeight: .infinity)
        .padding(.leading, -8)
        // A hack for list row insets not working. This hack also applies to the section header though.
    }
}


