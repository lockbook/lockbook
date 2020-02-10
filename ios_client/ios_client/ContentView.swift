//
//  ContentView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 1/30/20.
//  Copyright © 2020 Lockbook. All rights reserved.

import SwiftUI

struct ContentView: View {
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
                
                NavigationLink(destination: Text("test1")) {
                    MonokaiButton(text: "Create new Lockbook")
                }
                NavigationLink(destination: Text("test1")) {
                    MonokaiButton(text: "Import Lockbook")
                }
            }
            .navigationBarTitle("")
            .navigationBarHidden(true)
        }
    }
}

var documentsDirectory: String {
    return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.absoluteString
}

#if DEBUG
struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
#endif
