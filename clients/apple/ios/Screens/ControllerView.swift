//
//  ControllerView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/12/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ControllerView: View {
    @EnvironmentObject var coordinator: Coordinator

    var body: some View {
        switch coordinator.currentView {
            case .welcomeView: return AnyView(WelcomeView())
            case .fileBrowserView: return AnyView(
                VStack {
                    FileBrowserView()
                    ProgressWidget()
                        .frame(height: 20)
                        .padding()
                }
            )
            case .debugView: return AnyView(DebugView())
            case .none: return AnyView(Text("Nothing!"))
        }
    }
}

struct ControllerView_Previews: PreviewProvider {
    static var previews: some View {
        let coordinator = Coordinator()
        coordinator.currentView = .fileBrowserView
        
        return ControllerView().environmentObject(coordinator)
    }
}
