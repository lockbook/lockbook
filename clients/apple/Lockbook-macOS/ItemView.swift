//
//  ItemView.swift
//  macos
//
//  Created by Raayan Pillai on 5/31/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ItemView: View {
    var content: String
    var body: some View {
        Text(content)
    }
}

struct ItemView_Previews: PreviewProvider {
    static var previews: some View {
        ItemView(content: "Bunk!")
    }
}
