# C Bindings (v2)

This is a C API wrapper for lockbook-core. From this Rust library,
[`cbindgen`](https://github.com/eqrion/cbindgen) will create a nice C header file.

This differs from the current lockbook c_interface (v1) in a few key ways:

* raw C data structures are used to deliver and retrieve data over FFI instead of
  serializing & deserializing JSON.
* the C namespace is respected by prefixing types with `Lb`, functions with `lb_` and enum
  members and constants with `LB_`.
* no lazy static state: `init` returns an instance of `Core` as a raw pointer which should
  be the first argument to any core function call.
* [UUIDs are passed as 16 byte array values over
  FFI](https://github.com/steverusso/lockbook-x/pull/8) instead of as strings
  that panic if they can't be parsed.

## Missing endpoints

The following are `lockbook-core` endpoints that have yet to be implemented (roughly in
order of priority):

* `get_drawing`
* `save_drawing`
* `get_local_changes`
* `start_search`
* `create_link_at_path`
* `list_paths`
* `upgrade_account_google_play`
* `upgrade_account_app_store`

The `search_file_paths` endpoint will not be implemented as it's likely to become obsolete
in favor of `start_search`.
