//
//  ContentView.swift
//  macos
//
//  Created by Raayan Pillai on 5/25/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ContentView: View {
    var items: [String] = ["Hey", "Hi", "Ho"]
    var body: some View {
        NavigationView {
            List {
                ForEach(items, id: \.self) { item in
                    NavigationLink(destination: ItemView()) {
                        Text("Word: \(item)")
                    }
                }
            }
        }
    }
}


struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
