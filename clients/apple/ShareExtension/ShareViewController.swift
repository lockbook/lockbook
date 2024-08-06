//
//  ShareViewController.swift
//  ShareExtension
//
//  Created by Smail Barkouch on 7/31/24.
//

import SwiftUI
import UIKit
import UniformTypeIdentifiers
import MobileCoreServices

class ShareViewModel: ObservableObject {
    @Published var failed: Bool = false
    @Published var downloadUbig = false
    @Published var finished = false
}

// escape folder names with commas

class ShareViewController: UIViewController {

    var sharedItems: [Any] = []
    var processed: [String] = []
    let shareModel = ShareViewModel()
        
    override func viewDidLoad() {
        super.viewDidLoad()
        
        let contentView = UIHostingController(rootView: ShareExtensionView(shareModel: shareModel))
        self.addChild(contentView)
        self.view.addSubview(contentView.view)
        
        contentView.view.translatesAutoresizingMaskIntoConstraints = false
        contentView.view.topAnchor.constraint(equalTo: self.view.topAnchor).isActive = true
        contentView.view.bottomAnchor.constraint (equalTo: self.view.bottomAnchor).isActive = true
        contentView.view.leftAnchor.constraint(equalTo: self.view.leftAnchor).isActive = true
        contentView.view.rightAnchor.constraint (equalTo: self.view.rightAnchor).isActive = true
        
        DispatchQueue.global(qos: .userInitiated).async {
            
            if let eContext = self.extensionContext,
               let sharedFolder = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook")?.appendingPathComponent("shared") {
                if FileManager.default.fileExists(atPath: sharedFolder.path()) {
                    try! FileManager.default.removeItem(at: sharedFolder)
                }
                
                try! FileManager.default.createDirectory(at: sharedFolder, withIntermediateDirectories: true)
                
                self.processEContext(sharedFolder: sharedFolder, eContext: eContext)
                
                print("this is what got processed \(self.processed)")
                if self.processed.isEmpty {
                    self.shareModel.failed = true
                }
                
                if !self.shareModel.failed {
                    let filePathsQuery = self.processed.joined(separator: ",")
                    let shareURL = URL(string: "lb://sharedFiles?\(filePathsQuery.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed)!)")!
                    
                    self.shareModel.finished = true
                    eContext.completeRequest(returningItems: nil) { _ in
                        self.openURL(shareURL)
                    }
                }
                
                if self.shareModel.failed {
                    eContext.completeRequest(returningItems: [], completionHandler: nil)
                }
            }
        }
    }
    
    @objc
    @discardableResult
    func openURL(_ url: URL) -> Bool {
        var responder: UIResponder? = self
        while responder != nil {
            if let application = responder as? UIApplication {
                return application.perform(#selector(openURL(_:)), with: url) != nil
            }
            responder = responder?.next
        }
        return false
    }
    
    func processEContext(sharedFolder: URL, eContext: NSExtensionContext) {
        for input in eContext.inputItems {
            if let input = input as? NSExtensionItem {
                for attachment in input.attachments ?? [] {
                    processAttachment(sharedFolder: sharedFolder, attachment: attachment)
                }
            }
        }
        
        if processed.isEmpty {
            shareModel.failed = true
        }
    }
    
    func processAttachment(sharedFolder: URL, attachment: NSItemProvider) {
        if attachment.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
            print("got URL!")
            let semaphore = DispatchSemaphore(value: 0)

            let _ = attachment.loadObject(ofClass: URL.self) { (url, error) in
                if let url = url {
                    self.importFileIntoAppGroup(sharedFolder: sharedFolder, importing: url)
                } else {
                    print("failed?")
                }
                
                semaphore.signal()
            }
            
            semaphore.wait()

        } else if attachment.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
            let semaphore = DispatchSemaphore(value: 0)
            
            let _ = attachment.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { (url, error) in
                if let url = url {
                    self.importFileIntoAppGroup(sharedFolder: sharedFolder, importing: url)
                }

                semaphore.signal()
            }

            semaphore.wait()
        } else if attachment.hasItemConformingToTypeIdentifier(UTType.movie.identifier) {
            let semaphore = DispatchSemaphore(value: 0)

            let _ = attachment.loadFileRepresentation(forTypeIdentifier: UTType.movie.identifier) { (url, error) in
                if let url = url {
                    self.importFileIntoAppGroup(sharedFolder: sharedFolder, importing: url)
                }

                semaphore.signal()
            }
            
            semaphore.wait()
        } else {
            shareModel.failed = true
        }
    }
    
    func importFileIntoAppGroup(sharedFolder: URL, importing: URL) {
        let parent = sharedFolder.appendingPathComponent(UUID().uuidString)
        let newHome = parent.appendingPathComponent(importing.lastPathComponent)
                        
        do {
            try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: false)
                        
            try FileManager.default.copyItem(at: importing, to: newHome)

            self.processed.append(newHome.pathComponents.suffix(3).joined(separator: "/"))
        } catch {
            shareModel.failed = true
        }
    }
}
