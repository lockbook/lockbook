# SwiftLockbookCore

Everything you need to interface with the Lockbook library, in Swift.

## Usage

Ensure that the `liblb_rs.a` static library and `lb_rs.h` header files are in their appropriate destinations.

`liblb_rs.a` at `<PROJECT_ROOT>/clients/swift/CLockbookCore/Sources/CLockbookCore/lib`

`lb_rs.h` at `<PROJECT_ROOT>/clients/swift/CLockbookCore/Sources/CLockbookCore/include`

Then run the following

`swift build`

`swift test`

## Generate linux manifest

`swift test --generate-linuxmain`
