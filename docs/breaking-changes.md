# Version upgrades and breaking changes

Evolving the Lockbook code base requires some special considerations. 

## Breaking change between core versions

Because we support offline use our clients store and maintain local state. If we update our app, we need to make sure the new version of the app can gracefully interact with the current data on the device. Ultimately we can always nuke the local state and re-sync with the server. We want to avoid doing this where possible for the following reasons:

+ We would need to explicitly keep track of offline changes that haven't yet been synced
+ Depending on the user this could take a long time
+ Needlessly places a load our infrastructure

Where possible therefore, where possible we should support old functionality. When this is not possible (a data format changed), a "migration" must be performed. Encoded in rust forever will be instructions on how to step through each version of state. These migrations should not require an internet connection. All core endpoints that interact with state may return `MigrationRequired`, you can also explicitly check the status by calling `get_db_state`.

#### Breaking changes between versions of sled

Sled is the embedded db that lives on all client devices. Sled itself could have breaking changes between versions. The migration process for that is documented [here](https://docs.rs/sled/0.34.4/sled/struct.Db.html#method.export).

## Breaking changes between core and server

Because our clients are native ones installed via arbitrary channels (downloads from our site, package manager, app stores, or compiled from source), our server will explicitly need to keep track of what the minimum version client it can service. We want to be very conservative with incrementing the hard cut off where our server essentially says "I can't service this request until you update your client." Being forced to update is generally a bad experience, and it is made worse depending on how you need to update.

An example of a breaking change between client and server could include (but are not limited to):
+ Changing the name of an endpoint
+ Changing what inputs are required for an endpoint
+ Changing the output of an endpoint

Where possible we should strive to maintain backwards compatibility, but some reasons to force an update may include:
+ Increasing the reliability of our system (new endpoint eliminates a race condition but requires more information)
+ Reduction of complexity (make a new column `Non-Nullable`)
+ Ensuring that clients interoperate properly (if you upgrade 1 device, you may need to update the rest)
+ Security reasons (If we change our authentication scheme, supporting the old one long term would mean we never made auth better as an attacker could just pretend they're using the old client).

Again, this only effects on-line interactions, you would always be able to access and edit the content already on your system. You'll also always be able to export and use client side features, but to sync you may need to upgrade. 

To accomplish this: any server endpoint can return `UpdateRequired` which should make it's way to the UI ultimately signaling that they can't do the operation they were trying to do without an update.

## Internal Representation of Documents

Using a file extension lets us treat documents differently, but we could want to represent documents differently internally. Creating `DocumentType`s provides us a layer of flexibility in gracefully handling:
+ Different ways to store text (changelog, which captures a lot of detail about who made what edit when versus no history files which take up far less space.)
+ Different ways to represent drawings

This allows us to innovate on file formats while preserving backwards compatibility (old `.drawings` are still readable).
