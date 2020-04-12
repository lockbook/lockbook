//
//  ContentView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 1/30/20.
//  Copyright © 2020 Lockbook. All rights reserved.

import SwiftUI

struct WelcomeView: View {
    var lockbookApi: CoreApi
    @State public var navigationBarHidden = true
    
    var body: some View {
        NavigationView {
            VStack {
                HStack {
                    Spacer()
                }
                Text("lockbook")
                    .fontWeight(.light)
                    .font(.system(size: 45, design: .monospaced))
                    .padding(.bottom, 15)
                
                Text("Secure. Private. Reliable.")
                    .font(.system(size: 15, design: .monospaced))
                    .padding(.bottom, 100)
                
                NavigationLink(destination: NewLockbookView()) {
                    MonokaiButton(text: "New Lockbook")
                }
                
                NavigationLink(destination:
                    ListView(lockbookApi: lockbookApi)
                ) {
                            MonokaiButton(text: "Import Lockbook")
                }
            }.onAppear(perform: {
                self.navigationBarHidden = true
            })
            .onDisappear(perform: {
                self.navigationBarHidden = false
            })
        }
    }
}

var documentsDirectory: String {
    return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.absoluteString
}

#if DEBUG
struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        WelcomeView(lockbookApi: CoreApi())
    }
}
#endif
