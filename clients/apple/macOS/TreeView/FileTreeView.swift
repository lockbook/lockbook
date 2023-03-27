import SwiftUI
import SwiftLockbookCore

struct FileTreeView: NSViewRepresentable {

    let scrollView = NSScrollView()
    let treeView = MenuOutlineView()
    let delegate = TreeDelegate()
    var dataSource = DataSource()

    @EnvironmentObject var files: FileService
    @EnvironmentObject var currentSelection: CurrentDocument
    
    let previousFilesHash: Reference<Int?> = Reference(nil)
    let previousOpenDocumentHash: Reference<Int?> = Reference(nil)
            
    func makeNSView(context: Context) -> NSScrollView {        
        delegate.documentSelected = {
            if $0.fileType == .Document {
                DI.currentDoc.selectedDocument = $0
            } else {
                DI.currentDoc.selectedFolder = $0
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

        treeView.registerForDraggedTypes([NSPasteboard.PasteboardType(DataSource.REORDER_PASTEBOARD_TYPE)])

        // TODO changing this to true will allow us to accept drops from finder
        treeView.setDraggingSourceOperationMask(NSDragOperation(), forLocal: false)

        treeView.setDraggingSourceOperationMask(NSDragOperation.move, forLocal: true)
        
        let onlyColumn = NSTableColumn()
        onlyColumn.resizingMask = .autoresizingMask
        onlyColumn.minWidth = 100
        treeView.addTableColumn(onlyColumn)

        treeView.dataSource = dataSource
        treeView.delegate = delegate
        treeView.stronglyReferencesItems = true

        return scrollView
    }
    
    func updateNSView(_ nsView: NSScrollView, context: Context) {
        if previousFilesHash.value != files.idsAndFiles.hashValue {
            treeView.reloadData()
            previousFilesHash.value = files.idsAndFiles.hashValue
        }
        
        if previousOpenDocumentHash.value != currentSelection.selectedDocument?.hashValue {
            if let file = DI.currentDoc.selectedDocument {
                scrollAndexpandAncestorsOfDocument(file: file)
            }
            
            treeView.selectRowIndexes(IndexSet(integer: treeView.row(forItem: DI.currentDoc.selectedDocument)), byExtendingSelection: false)
            
            previousOpenDocumentHash.value = currentSelection.selectedDocument?.hashValue
        }
    }
    
    func scrollAndexpandAncestorsOfDocument(file: File) {
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
            if  isItemExpanded(clickedItem) {
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

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

}

class Reference<T> {
    var value: T
    init(_ value: T) { self.value = value }
}
