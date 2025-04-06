import SwiftUI
import SwiftWorkspace

struct FileTreeView: NSViewRepresentable {
    @EnvironmentObject var filesModel: FilesViewModel
    
    func makeNSView(context: Context) -> some NSView {
        let scrollView = NSScrollView()
        scrollView.autoresizingMask = [.width, .height]
        scrollView.hasVerticalScroller = true
        
        let treeView = NSOutlineView(frame: scrollView.bounds)
        treeView.autoresizingMask = [.width, .height]
        treeView.headerView = nil
        
        let column = NSTableColumn()
        column.title = "Name"
        column.isEditable = false
        treeView.addTableColumn(column)
        treeView.outlineTableColumn = column
                
        scrollView.documentView = treeView
        return scrollView
    }
    
    func updateNSView(_ nsView: NSViewType, context: Context) {
        
    }
}

class FileTreeDataSource: NSObject, NSOutlineViewDataSource, NSPasteboardItemDataProvider {
    @ObservedObject var filesModel: FilesViewModel
    
    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
    }
    
    var dragged: [File]? = nil
    var lastFilesHash: Int? = nil
    var selectedDoc: UUID? = nil
    
    func outlineView(
            _ outlineView: NSOutlineView,
            numberOfChildrenOfItem item: Any?
    ) -> Int {
        guard let file = item as? File else {
            return 0
        }
        
        let children = filesModel.childrens[file.id] ?? []
        
        return children.count
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            isItemExpandable item: Any
    ) -> Bool {
        guard let file = item as? File else {
            return false
        }
        
        let children = filesModel.childrens[file.id] ?? []

        return file.type == .folder && !children.isEmpty
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            child index: Int,
            ofItem item: Any?
    ) -> Any {
        let file = item as! File
        let children = filesModel.childrens[file.id] ?? []
        
        return children[index]
    }

    func outlineView(_ outlineView: NSOutlineView, pasteboardWriterForItem item: Any) -> NSPasteboardWriting? {
        let pb = NSPasteboardItem()
        let file = item as! File
        
        pb.setData(try! JSONEncoder().encode(file), forType: NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE))
        pb.setDataProvider(self, forTypes: [NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE), .fileURL])

        return pb
    }

    func outlineView(_ outlineView: NSOutlineView, draggingSession session: NSDraggingSession, willBeginAt screenPoint: NSPoint, forItems draggedItems: [Any]) {
        dragged = draggedItems as? [File]
        
        session.draggingPasteboard.setData(try! JSONEncoder().encode(dragged), forType: NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE))
    }

    func outlineView(_ outlineView: NSOutlineView, validateDrop info: NSDraggingInfo, proposedItem item: Any?, proposedChildIndex index: Int) -> NSDragOperation {
        let parent = item == nil ? DI.files.root! : item as! File
        if parent.type == .document {
            return []
        }
        
        if (info.draggingSource as? NSOutlineView) === outlineView {
            return NSDragOperation.move
        } else {
            return NSDragOperation.copy
        }
    }

    func outlineView(_ outlineView: NSOutlineView, acceptDrop info: NSDraggingInfo, item: Any?, childIndex index: Int) -> Bool {
        let parent = item == nil ? DI.files.root! : item as! File
        
        if (info.draggingSource as? NSOutlineView) === outlineView {
            return DI.files.moveFiles(ids: dragged!.map({ $0.id }), newParent: parent.id)
        } else {
            guard let urls = info.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL] else {
                return false
            }
            
            if(parent.type == .document) {
                return false
            }
            
            let parent = item == nil ? DI.files.root! : item as! File
            
            return DI.importExport.importFilesSync(sources: urls.map({ url in url.path(percentEncoded: false)}), destination: parent.id)
        }
    }
    
    func pasteboard(_ pasteboard: NSPasteboard?, item: NSPasteboardItem, provideDataForType type: NSPasteboard.PasteboardType) {
        if(type == .fileURL) {
            let files = try! JSONDecoder().decode([File].self, from: item.data(forType: NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE))!)
            
            pasteboard?.clearContents()
            var pasteboardItems: [NSPasteboardItem] = []

            for file in files {
                if let dest = DI.importExport.exportFilesToTempDirSync(meta: file) {
                    let newItem = NSPasteboardItem()
                    newItem.setData(dest.dataRepresentation, forType: .fileURL)
                    pasteboardItems.append(newItem)
                }
            }
            
            pasteboard?.writeObjects(pasteboardItems)
        }
    }
    
    func outlineView(_ outlineView: NSOutlineView, draggingSession session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation) {
        if operation == .move {
            dragged = nil
        }
    }

    static let REORDER_PASTEBOARD_TYPE = "net.lockbook.metadata"
}

