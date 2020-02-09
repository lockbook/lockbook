//
//  ContentView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 1/30/20.
//  Copyright © 2020 Lockbook. All rights reserved.

import SwiftUI

struct ContentView: View {
    var s: String
    
    var body: some View {
        Text(s)
    }
}
 
func getName() -> String {
//    let result = hello(documentsDirectory)
//    let sr =  String(cString: result!)
//    // IMPORTANT: once we get the result we have to release the pointer.
//    hello_release(UnsafeMutablePointer(mutating: result))
    return "sr"
}

var documentsDirectory: String {
    return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.absoluteString
}

#if DEBUG
struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView(s: "live preview")
    }
}
#endif
