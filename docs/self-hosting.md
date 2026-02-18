# Self Hosting Lockbook
 Lockbook is designed to be maximally user centric. Part of that goal involves the ability to host our server on your own infrastructure. This document explores benefits, drawbacks, and provides operational instructions.

## Benefits
* Greater control over hardware and costs
* Potentially more privacy

## Drawbacks
* Not being able to collaborate with users on the main Lockbook server (or any other server). See more below.
* Greater operational responsibility. Obviously, now you're responsible for backing up data, reading through our changelog, and applying any manual steps that an upgrade may require.
* Not supporting the development team working towards improving your experience. Consider donating:
  * BTC: `bc1qxewa56jvp59lg7vd3u8f23trvyjemjcc5npte9`
  * XMR: `47BqNSL4WdQAcff68wHALo5Ui6Ve8oZXYV1w16VZ7ZDV8EW1rGraok5d3jZiwz9SyYiyXDoGdGymCGNRCC1nLFDVFBSSvK1`

# Running the Server
* Dependencies: the rust toolchain. See `rustup.rs`.
* `lbdev` has a `server` subcommand which lets you hit the ground running. If you don't have `lbdev` installed you can run `cargo r -p lbdev server` from the root of our project.
* The server will log where it's reachable. 

## Configuring the server
Our server is configurable via environment variables. The [`config.rs`](https://github.com/lockbook/lockbook/blob/master/server/src/config.rs) file is optimized to document the env vars we use. Some notable variables are:
  * `ENVIRONMENT` allows you to put the server in `PROD` mode, binding to `0.0.0.0` instead of `127.0.0.1` to allow external traffic to reach the server, but also requiring you to provide `SSL_CERT_LOCATION` and `SSL_PRIVATE_KEY_LOCATION`.
  * You can update the `local.env` that `lbdev` launches the server with. Once you have a configuration that meets your goals you can move away from `lbdev`. In production we use this `systemd` service specification: https://github.com/lockbook/lockbook/blob/master/server/etc/systemd/system/lockbook-server.service.

## Configuring a client
* Clients with env vars easily accessible can be pointed at a different server during account creation / login by setting the `API_URL` environment variable. Server logs will indicate whether the account was created in the right place. All lockbook clients expose the concept of `debug_info`, generally in settings which displays the current `server_url` as another mechanism of debugging. 
* Clients that are difficult to work with (Android / iOS) have *Advanced* sections of onboarding that allow you to specify an `API_URL`. 

# Long term vision
It's always been part of our vision for Lockbook to support federation, allowing people to interact seamlessly with people on other servers.

However, the design of our product treats our own server as a hostile entity because even we do not trust our cloud provider. As such we have done everything we can in our control to minimize the information that is sent to our server, and clients are designed to be resilient against server downtime (we call this offline support).

Most people self host as a mitigation for problems that we've solved fundementally for the whole platform. So therefore we've found that it's mostly something people ask us out-of-instinct.

If your situation demands self-hosting Lockbook at scale and you'd like an enhanced level of support, don't hesitate to reach out to us!
* Discord: https://discord.gg/lockbook
* Email: parth@lockbook.net