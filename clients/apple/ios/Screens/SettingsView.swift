//
//  DebugView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var debugger: Debugger
    @ObservedObject var coordinator: Coordinator
    
    func fail() -> Void {
        print("Failure!")
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: 15) {
            Spacer()
            Group {
                Text("Actions")
                Button(action: {
                    self.coordinator.sync()
                }) {
                    HStack {
                        Image(systemName: "arrow.up.arrow.down.circle.fill")
                        Text("Sync")
                    }
                    .foregroundColor(.green)
                }
                Button(action: {
                    self.coordinator.incrementalSync()
                }) {
                    HStack {
                        Image(systemName: "arrow.up.and.down.circle.fill")
                        Text("Incremental Sync")
                    }
                    .foregroundColor(.yellow)
                }
                Button(action: {
                    self.coordinator.reloadFiles()
                }) {
                    HStack {
                        Image(systemName: "arrow.2.circlepath.circle.fill")
                        Text("Reload Files")
                    }
                    .foregroundColor(.pink)
                }
            }
            Divider()
            Group {
                Text("Debugger")
                Button(action: {
                    if case .success(let username) = self.debugger.lockbookApi.getAccount() {
                        print("Username \(username)")
                    } else {
                        self.fail()
                    }
                }) {
                    HStack {
                        Image(systemName: "person.circle.fill")
                        Text("Dump Account")
                    }
                    .foregroundColor(.purple)
                }
            }
            Divider()
            Group {
                Text("Toggles")
                Button(action: {
                    self.coordinator.toggleAutoSync()
                }) {
                    HStack {
                        Image(systemName: "goforward.30")
                        Text("Auto-Syncing")
                    }
                    .foregroundColor(self.coordinator.autoSync ? .blue : .secondary)
                }
                Button(action: {
                    self.coordinator.toggleIncrementalAutoSync()
                }) {
                    HStack {
                        Image(systemName: "rays")
                        Text("Incremental-Syncing")
                    }
                    .foregroundColor(self.coordinator.incrementalAutoSync ? .blue : .secondary)
                }
            }
            Divider()
            Group {
                Text("Info")
                HStack {
                    Image(systemName: "globe")
                    Text(self.debugger.lockbookApi.getApiLocation())
                }
            }
            Spacer()
        }
        .padding(.horizontal, 50)
        .navigationBarTitle("Settings")
    }
}

struct DebugView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            NavigationView {
                SettingsView(coordinator: Coordinator()).environmentObject(Debugger())
            }.preferredColorScheme(.dark)
            /// Don't forget to checkout the light theme :D
//            NavigationView {
//                DebugView(coordinator: Coordinator()).environmentObject(Debugger())
//            }
        }
    }
}
