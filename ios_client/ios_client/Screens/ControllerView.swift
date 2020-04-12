//
//  ControllerView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/12/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ControllerView: View {
    var lockbookApi: LockbookApi
    @EnvironmentObject var screenCoordinator: ScreenCoordinator

    var body: some View {
        switch screenCoordinator.currentView {
            case .welcomeView: return AnyView(WelcomeView(lockbookApi: lockbookApi))
            case .createAccountView: return AnyView(CreateAccountView(lockbookApi: lockbookApi))
            case .listView: return AnyView(ListView(lockbookApi: lockbookApi))
            case .none: return AnyView(Text("Nothing!"))
        }
    }
}

struct ControllerView_Previews: PreviewProvider {
    static var previews: some View {
        ControllerView(lockbookApi: FakeApi()).environmentObject(ScreenCoordinator())
    }
}
