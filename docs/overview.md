## Conceptual Entities

The design is informed by the desire for clients to be capable without a network connection, and for file/network io to be minimizable when required. Clients should be able to say: which are the files I need to re-download, these are the files I don't want to maintain a local copy of. Where a client keeps their files and what they name it shouldn't be exposed to the server, but it should be synced across their devices. When users share files, it should allow the person receiving that file to determine where it ends up. It should allow a team of people to share a folder.

The architecture includes:

+ Clients - file editing and cryptographic operations.
+ API Nodes - crypographic verification, file index maintenance and querying.
+ File Index (db) - who owns what and when was it last modified.
+ File Contents (s3) - cheap and durable storage.

### File Content Store

This is the simplest of components to talk about. API nodes write to an S3 CRUD interface if the operation is authorized, and allow anyone to read from the S3 interface. The concerns of S3 causing vendor lockin is greatly diminished by the availability of "S3 API compatible vendors", in fact lockbook.app will likely use Digital Ocean's [S3 variant](https://www.digitalocean.com/products/spaces/). S3 is attractive because it's inexpensive, highly reliable (from a durability perspective) and files can be stored on their built in CDN's for super fast GET's directly from clients. No one but API nodes can write to the content store, there are ways to allow clients to interact upload directly using things like [presigned urls](https://docs.aws.amazon.com/AmazonS3/latest/dev/ShareObjectPreSignedURL.html), but that complicated the design of API nodes for things like collaboration. All the content is addressed by the `ID` field of the `FileDescription` and the file content is obviously encrypted. 

It should be reasonably trivial to allow people to publish unencrypted content from Lockbook (think medium style markdown blogs), and allow their content to be link addressable, but this is not on the timeline for now.

Clients will maintain a local content store which is an unencrypted version of this content store. 

### File Index

The file index will be a single table with this conceptual schema.

```
FileDescription {
	owner:		HashedString,
	id:		    UUID, 
	name: 		EncryptedString,
	path:		EncryptedString,
	version:	ServerTimestamp,
	sharedWith:	[]HashedString,
}
```

API nodes will use this to manage the state of the `Content Store`. For a client that's trying to figure out which files in their local content store are out of date, they'll ask the API node for "all files that are updated after point X that are owned by me". API nodes will query this store of state for that information.

The db will also store a `UserInfo` table which associates `Username` (hashed), `public key`, and `quota` information.

### API Nodes 

File Index integrity is maintained by API nodes. This basically ensures that if a client's updates a file, the Index reflects this, and when that client opens their laptop they are served the most recent version of the file. If there's a strange race condition the API nodes will reject requests that may cause data corruption (detailed below).

Clients will have keypairs on their device;, all side-effecting operations require a signature. API nodes will verify these signatures against the user's public key. API nodes use these signatures to ensure that these file operations are being made by the owners of the file.

### Clients

Clients generate keys locally and perform all operations by using these keys. Keys are synced directly between clients by using QR codes or manual entry. Clients will maintain local versions of the above datastores -- they'll save the files they care about locally and maintain an index of these files.

To figure out which files need updating, a client can pass a `LastUpdated` `ServerTimestamp` that it stored the last time it updated. The server will return every file greater then this timestamp.

When a user edits a file and is ready to push that change upto the server, they'll specify which version they have been editing. If someone edited the file while you were also editing it, the server will reject your edit. This will indicate you'll need to resync this file, and merge the differences between what you have and what the server has. After you've merged, you can retry and will be successful as you'll hand the server a more recent timestamp. Whenever possible, this process will happen automatically, and you'll only resolve merge conflicts - git style.
