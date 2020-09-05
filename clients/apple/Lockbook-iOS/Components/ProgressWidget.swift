//
//  ProgressWidget.swift
//  ios
//
//  Created by Raayan Pillai on 7/5/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ProgressWidget: View {
    @ObservedObject var coordinator: Coordinator
    var height: CGFloat = 5
    
    var body: some View {
        return GeometryReader { geometry in
            self.coordinator.progress.map { prog in
                Button(action: {
                    self.coordinator.progress = Optional.none
                }) {
                    VStack {
                        ZStack(alignment: .leading) {
                            Rectangle()
                                .frame(width: geometry.size.width, height: self.height)
                                .opacity(0.2)
                            Rectangle()
                                .frame(width: min(geometry.size.width * CGFloat(prog.0), geometry.size.width), height: self.height)
                                .animation(.linear)
                        }.cornerRadius(10)
                        HStack {
                            if (prog.0 == 0) {
                                Image(systemName: "checkmark.circle")
                            } else {
                                Image(systemName: "arrow.up.arrow.down.circle")
                            }
                            Text(prog.1)
                        }
                        .animation(.easeIn)
                    }
                    .foregroundColor(prog.2)
                }
            }
        }
        
    }
}

struct ProgressWidget_Previews: PreviewProvider {
    static var previews: some View {
        ProgressWidget(coordinator: Coordinator())
            .previewLayout(.fixed(width: 300, height: 50))
    }
}
