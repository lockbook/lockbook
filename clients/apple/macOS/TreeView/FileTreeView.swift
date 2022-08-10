import SwiftUI
import SwiftLockbookCore

struct FileTreeView: NSViewRepresentable {

    let scrollView = NSScrollView()
    let treeView = MenuOutlineView()
    let delegate = TreeDelegate()
    var dataSource = DataSource()

    @EnvironmentObject var files: FileService

    func makeNSView(context: Context) -> NSScrollView {
        
        delegate.documentSelected = {
            DI.currentDoc.selectedItem = $0
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
        treeView.reloadData()
        // Should this happen in the delegate?
        for row in 0...treeView.numberOfRows {
            if let item = treeView.item(atRow: row) as? File {
                if let selection = DI.currentDoc.selectedItem {
                    if item.id == selection.id {
                        treeView.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
                    }
                }
            }
        }
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
