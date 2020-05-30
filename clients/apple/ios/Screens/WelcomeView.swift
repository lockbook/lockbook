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
                Text("lockbook")
                    .fontWeight(.light)
                    .font(.system(size: 45, design: .monospaced))
                    .padding(.bottom, 15)
                
                Text("Secure. Private. Reliable.")
                    .font(.system(size: 15, design: .monospaced))
                    .padding(.bottom, 100)
                
                NavigationLink(destination: CreateAccountView()) {
                    MonokaiButton(text: "New Lockbook")
                }
                NavigationLink(destination: ImportAccountView()) {
                    MonokaiButton(text: "Import Lockbook")
                }
            }
        }
    }
}

struct WelcomeView_Previews: PreviewProvider {
    static var previews: some View {
        WelcomeView().environmentObject(Coordinator())
    }
}
