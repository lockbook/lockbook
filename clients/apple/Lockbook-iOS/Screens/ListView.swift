//
//  ListView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FileBrowser: View {
    @EnvironmentObject var coordinator: Coordinator
    
    var body: some View {
        GeometryReader{ geometry in
            NavigationView {
                PullDownView(width: geometry.size.width, height: geometry.size.height)
            }
        }
    }
}

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
        let childView = UIHostingController(rootView: FolderView(dirId: self.coordinator.getRoot(), dirName: "\(coordinator.username)'s Files").environmentObject(self.coordinator))
        childView.view.frame = CGRect(x: 0, y: 0, width: width, height: height)
        
        control.addSubview(childView.view)
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


struct ListView_Previews: PreviewProvider {
    static var previews: some View {
        FileBrowser().preferredColorScheme(.dark).environmentObject(Coordinator())
    }
}
