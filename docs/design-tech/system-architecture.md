# Conceptual Entities

### FileDb - S3

FileDb is a key value store optimized for storing arbitary objects. This is where all the encrypted content lives.

### IndexDb - Redis

Database API Nodes use to keep track of users and files. For every user, we store their `username` and their `public_key`. For each file, we store:

+ The location of the encrypted contents within FileDb
+ Version information (primarily to avoid race conditions)
+ Various access levels and ownership information related to the file
+ The schema is documented [here](redis-schema.md).

### API Nodes 

IndexDb and FileDb integrity is maintained by API nodes.

+ Has this person cryptographically demonstrated that they possess the `private_key` associated with the operation they're trying to do?
+ Is this person updating a file they don't have the most recent version of?

The contract between `lb-rs` and `server` is specified in [Rust](https://github.com/lockbook/lockbook/blob/master/libs/lb/lb-rs/libs/shared/src/api.rs) and checked at compile time.

### Clients

Clients are responsible for all cryptographic operations (key generation, encryption, decryption & signing). This is a lb-rs component of Lockbook's security model.

Clients also maintain local copies of all the files relevant to them. All operations are possible offline.

When clients come online they figure out what operations will bring them in sync with Lockbook's backend. This complexity is documented [here](sync.md).
