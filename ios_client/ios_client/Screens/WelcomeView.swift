//
//  ContentView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 1/30/20.
//  Copyright © 2020 Lockbook. All rights reserved.

import SwiftUI

struct WelcomeView: View {
    @State public var navigationBarHidden = true
    
    var body: some View {
        NavigationView {
            VStack {
                HStack {
                    Spacer()
                }
                Text("Lockbook")
                    .fontWeight(.light)
                    .font(.system(size: 45, design: .monospaced))
                    .padding(.bottom, 15)
                
                Text("Secure. Private. Reliable.")
                    .padding(.bottom, 100)
                
                NavigationLink(destination: NewLockbookView()) {
                    MonokaiButton(text: "New Lockbook")
                }
                
                NavigationLink(destination:
                    Text("Unimplemented")
                        .navigationBarTitle("Import Lockbook")) {
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
        WelcomeView()
    }
}
#endif
