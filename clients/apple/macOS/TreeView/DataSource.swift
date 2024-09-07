import AppKit
import SwiftLockbookCore

// Reference for drag and drop:
// https://github.com/KinematicSystems/NSOutlineViewReorder/blob/master/OutlineViewReorder/OutlineDataSource.swift
class DataSource: NSObject, NSOutlineViewDataSource, NSPasteboardItemDataProvider {

    var dragged: [File]? = nil
    var lastFilesHash: Int? = nil
    var selectedDoc: UUID? = nil
    
    func outlineView(
            _ outlineView: NSOutlineView,
            numberOfChildrenOfItem item: Any?
    ) -> Int {
        let file = item as? File
        let children = DI.files.childrenOf(file)
        return children.count
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            isItemExpandable item: Any
    ) -> Bool {
        let file = item as! File

        return file.fileType == .Folder
                && !DI.files.childrenOf(file).isEmpty
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            child index: Int,
            ofItem item: Any?
    ) -> Any {
        let parent = item as? File
        let siblings = DI.files.childrenOf(parent)
        let node = siblings[index]
        return node
    }

    func outlineView(_ outlineView: NSOutlineView,
                     pasteboardWriterForItem item: Any) -> NSPasteboardWriting? {
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
        if parent.fileType == .Document {
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
            
            if(parent.fileType == .Document) {
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

class TreeDelegate: NSObject, MenuOutlineViewDelegate { 
    func outlineView(_ outlineView: NSOutlineView, menuForItem item: Any?) -> NSMenu? {
        let menu = NSMenu()
        let file = item as! File
        let parent = item == nil ? DI.files.root! : file
        
        if let selectedFiles = DI.selected.selectedFiles,
           selectedFiles.contains(file),
           selectedFiles.count > 1 {
            let selectedFiles = DI.selected.getConsolidatedSelectedFiles()
            
            menu.addItem(Delete(files: selectedFiles))
            menu.addItem(ShareMultipleTo(files: selectedFiles, fileTree: outlineView))
            
            return menu
        }
        
        if parent.fileType == .Folder {
            menu.addItem(CreateDocument(file: parent))
            menu.addItem(CreateDrawing(file: parent))
            menu.addItem(CreateFolder(file: parent))
        }

        if parent.id != parent.parent {
            menu.addItem(RenameFile(file: parent))
            menu.addItem(Share(file: parent))
            menu.addItem(ShareExternallyMenu(file: parent, fileTree: outlineView))
            menu.addItem(Delete(files: [parent]))
        }
        
        return menu
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            viewFor tableColumn: NSTableColumn?,
            item: Any
    ) -> NSView? {
        let file = item as! File
        return FileItemView(file: file)
    }

    func outlineView(_ outlineView: NSOutlineView,
                     shouldSelectItem item: Any) -> Bool {
        return false
    }
}

