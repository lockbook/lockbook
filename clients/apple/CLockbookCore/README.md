# CLockbookCore

At the moment SPM (Swift Package Manager) doesn't allow for mixed-language packages.

A way to circumvent this is dedicating on module to "C code" and importing it into your "Swift code" module.

This is the "C code" module for the Lockbook core library.

It points to the built library file (`liblockbook_core_external_interface.a`) and exposes its functionality.

The `dummy.c` file is so that Swift thinks this is actually a C library. 

This can be built manually with `swift build`
