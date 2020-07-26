//
//  PullDownView.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct PullDownView : UIViewRepresentable {
    @EnvironmentObject var coordinator: Coordinator

    var width : CGFloat
    var height : CGFloat
    
    func makeCoordinator() -> SVCoordinator {
        SVCoordinator(self)
    }
    
    func makeUIView(context: Context) -> UIScrollView {
        let control = UIScrollView()
        control.refreshControl = UIRefreshControl()
        control.refreshControl?.addTarget(context.coordinator, action: #selector(SVCoordinator.handleRefreshControl), for: .valueChanged)
        if let root = self.coordinator.getRoot() {
            let childView = UIHostingController(rootView: FolderList(coordinator: self.coordinator, dirId: root, dirName: "\(self.coordinator.account.username)'s Files"))
            childView.view.frame = CGRect(x: 0, y: 0, width: width, height: height)
            control.addSubview(childView.view)
        } else {
            let childView = UIHostingController(rootView: Text("Something has gone horribly wrong..."))
            childView.view.frame = CGRect(x: 0, y: 0, width: width, height: height)
            control.addSubview(childView.view)
        }
        return control
    }
    
    func updateUIView(_ uiView: UIScrollView, context: Context) {
    }
    
    class SVCoordinator: NSObject {
        var control: PullDownView
        
        init(_ control: PullDownView) {
            self.control = control
        }
        @objc func handleRefreshControl(sender: UIRefreshControl) {
            self.control.coordinator.sync()
            sender.endRefreshing()
        }
    }
}

struct PullDownView_Previews: PreviewProvider {
    static var previews: some View {
        GeometryReader { geometry in
            PullDownView(width: geometry.size.width, height: geometry.size.height).environmentObject(Coordinator())
        }
    }
}
