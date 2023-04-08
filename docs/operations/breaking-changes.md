# Version Upgrades and Breaking Changes

Evolving the Lockbook codebase requires some special considerations. 

## Breaking Changes Between Core Versions

Our clients store and maintain local state because we support offline use. If we update our app, we need to make sure the new version can gracefully interact with the current data on the device. We want to avoid nuking the local state and re-syncing with the server where possible for the following reasons:

+ Unsynced offline changes need to be maintained or discarded
+ Re-syncing with the server can take a long time
+ Re-syncing with the server places needless load on our infrastructure

Therefore, where possible, we should support the current data format with the new app version. When this is not possible (data format changed), a 'migration' must be performed, which converts local data from the format of one app version to the next. These migrations should not require an internet connection. When core is initialized, it performs whatever migrations are necessary automatically.

## Breaking Changes Between Core and Server

Our server will explicitly need to keep track of the minimum version client it can service because our clients are installed via arbitrary channels (downloads from our site, package manager, app stores, or compiled from source). We aim to support old app versions as able because being forced to update is a bad user experience. Sometimes we will deem a backwards-incompatible API change to be necessary despite its impact on user experience, for purposes such as:
+ Increasing the reliability of our system (new endpoint eliminates a race condition but requires more information)
+ Reduction of complexity (e.g. making an optional field required)
+ Ensuring that clients interoperate properly (if you update one device and it migrates files to a new format, you will need to update the rest)
+ Improving security (we effectively can't improve our authentication scheme if an attacker can use old schemes)

Again, this only affects online interactions, you would always be able to access and edit the content already on your system. You'll also always be able to export and use client-side features, but you may need to update to sync. 

To accomplish this: any server endpoint can return `UpdateRequired` which is ultimately raised to the user, signaling that they can't do the operation they were trying to do without an update.

## Internal Representation of Documents

Using a file extension lets us treat documents differently, but we could want to represent documents differently internally. Creating `DocumentType`s provides us a layer of flexibility in gracefully handling:
+ Different ways to store text (changelog, which captures a lot of detail about who made what edit when versus no history files which take up far less space.)
+ Different ways to represent drawings

This allows us to innovate on file formats while preserving backward compatibility (old `.drawings` are still readable).
