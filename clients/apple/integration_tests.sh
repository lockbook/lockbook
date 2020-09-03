xcodebuild -project clients/apple/lockbook.xcodeproj \
            -scheme LockbookCore \
            -destination platform=iOS\ Simulator,OS=13.6,name=iPhone\ 11 \
            clean test
