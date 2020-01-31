//
//  ContentView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 1/30/20.
//  Copyright © 2020 Lockbook. All rights reserved.

import SwiftUI

struct ContentView: View {
    let s = getName()
    var body: some View {
        Text(s)
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
 
func getName() -> String {
    let result = hello("Parth")
    let sr =  String(cString: result!)
    // IMPORTANT: once we get the result we have to release the pointer.
    hello_release(UnsafeMutablePointer(mutating: result))
    return sr
}
