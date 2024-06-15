import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore

struct FileTreeView: NSViewRepresentable {

    let scrollView = NSScrollView()
    let treeView = MenuOutlineView()
    let delegate = TreeDelegate()
    var dataSource = DataSource()
    
    @Binding var expandedFolders: [File]
    @Binding var lastOpenDoc: File?

    @EnvironmentObject var files: FileService
    @EnvironmentObject var workspace: WorkspaceState
    
    let previousFilesHash: Reference<Int?> = Reference(nil)
    let previousOpenDocumentHash: Reference<Int?> = Reference(nil)
    
    func makeNSView(context: Context) -> NSScrollView {
        if treeView.numberOfColumns != 1 {
            delegate.documentSelected = { meta in
                if meta.fileType == .Document {
                    DI.workspace.requestOpenDoc(meta.id)
                } else if meta.fileType == .Folder {
                    DI.workspace.selectedFolder = meta.id
                }
            }
            
            delegate.folderExpandedCollapsed = { meta, expanded in
                if(expanded) {
                    expandedFolders.append(meta)
                } else {
                    if let index = expandedFolders.firstIndex(of: meta) {
                        expandedFolders.remove(at: index)
                    }
                }
            }
                        
            scrollView.documentView = treeView
            scrollView.hasVerticalScroller = true
            scrollView.horizontalScrollElasticity = .none
            scrollView.hasHorizontalScroller = false
            scrollView.hasHorizontalRuler = false
            scrollView.drawsBackground = false
            
            treeView.autoresizesOutlineColumn = true
            treeView.headerView = nil
            treeView.usesAutomaticRowHeights = true
            
            treeView.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
            
            treeView.setDraggingSourceOperationMask(.copy, forLocal: false)
            treeView.setDraggingSourceOperationMask(.move, forLocal: true)
            
            treeView.registerForDraggedTypes([NSPasteboard.PasteboardType(DataSource.REORDER_PASTEBOARD_TYPE), .fileURL])
            
            let onlyColumn = NSTableColumn()
            onlyColumn.resizingMask = .autoresizingMask
            onlyColumn.minWidth = 100
            treeView.addTableColumn(onlyColumn)
            
            treeView.dataSource = dataSource
            treeView.delegate = delegate
            treeView.stronglyReferencesItems = true
            
            for item in expandedFolders {
                treeView.expandItem(item)
            }
            
            scrollAndSelectDoc(maybeOpenDocId: workspace.openDoc)
        }

        return scrollView
    }
    
    func updateNSView(_ nsView: NSScrollView, context: Context) {
        if previousFilesHash.value != files.idsAndFiles.hashValue {
            previousFilesHash.value = files.idsAndFiles.hashValue
            treeView.reloadData()
        }
        
        let maybeOpenDocId = workspace.openDoc
                
        if lastOpenDoc?.id != maybeOpenDocId {
            scrollAndSelectDoc(maybeOpenDocId: maybeOpenDocId)
            
            if let openDocId = maybeOpenDocId {
                lastOpenDoc = DI.files.idsAndFiles[openDocId]
            } else {
                lastOpenDoc = nil
            }
        }
    }
    
    func scrollAndSelectDoc(maybeOpenDocId: UUID?) {
        if let openDocId = maybeOpenDocId {
            if let openDoc = DI.files.idsAndFiles[openDocId] {
                scrollAndExpandAncestorsOfDocument(file: openDoc)
                treeView.selectRowIndexes(IndexSet(integer: treeView.row(forItem: openDoc)), byExtendingSelection: false)
            }
        } else {
            treeView.selectRowIndexes(IndexSet(), byExtendingSelection: false)
        }
    }
    
    func scrollAndExpandAncestorsOfDocument(file: File) {
        if(treeView.row(forItem: file) == -1) {
            let pathToRoot = DI.files.filesToExpand(pathToRoot: [], currentFile: file)
                    
            for parent in pathToRoot {
                treeView.animator().expandItem(parent)
            }
        }
        
        treeView.animator().scrollRowToVisible(treeView.row(forItem: file))
    }
}

protocol MenuOutlineViewDelegate : NSOutlineViewDelegate {
    func outlineView(_ outlineView: NSOutlineView, menuForItem item: Any?) -> NSMenu?
}

class MenuOutlineView: NSOutlineView {

    init() {
        super.init(frame: .zero)
        target = self
        action = #selector(outlineViewClicked(_:))
    }

    @objc private func outlineViewClicked(_ outlineView: NSOutlineView) {
        if let clickedItem = item(atRow: clickedRow) {
            if isItemExpanded(clickedItem) {
                animator().collapseItem(clickedItem)
            } else {
                animator().expandItem(clickedItem)
            }
        }
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        let point = self.convert(event.locationInWindow, from: nil)
        let row = self.row(at: point)
        let item = item(atRow: row)

        return (delegate as! MenuOutlineViewDelegate).outlineView(self, menuForItem: item)
    }
    
    override var acceptsFirstResponder: Bool {
        return false
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

}

class Reference<T> {
    var value: T
    init(_ value: T) { self.value = value }
}
