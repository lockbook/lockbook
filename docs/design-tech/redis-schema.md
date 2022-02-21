# Redis Schema

We use `redis` as our primary data store. We heavily rely on [redis transactions](https://redis.io/topics/transactions)
as we're very often checking-and-setting values. We use [`redis_utils`](https://github.com/parth/redis_utils) to manage
these transactions.

Keys generally follow the format `key_type:key:value_type`.

Values are json encoded.

# Accounts:

+ `username:x:public_key`
    + `x` is the username.
+ `public_key:x:username`
    + `x` is
      the [compressed](https://docs.rs/libsecp256k1/0.7.0/libsecp256k1/struct.PublicKey.html#method.serialize_compressed)
      base64 encoded version of a `libsecp256k1` public key. All `public_key:...` keys use this format.
+ `public_key:x:owned_files`
    + `x` is the `public_key`.
    + The value is a list of all the files owned by a particular user, we `WATCH` this key in situations where we want
      to conceptually acquire a lock on all the files a user owns. We'll likely perform an `MGET` following this call to
      get all these files.
+ `public_key:x:data_cap`
    + `x` is the `public_key`.
    + Data cap refers to the maximum amount of space a user can store in lockbook.

# Filemetadata

+ `file_id:x:metadata`
    + `x` is the `uuid` of the file.
+ `file_id:x:size`
  + `x` is the `uuid` of the file.
  + size (value) is a json encoded `FileUsage` object 

# Stripe Billing

+ `public_key:x:stripe_user_info`
  + `x` is the `public_key`.
  + Basic stripe information for a user.
+ `stripe_customer_id:x:public_key`.
  + `x` is the stripe `customer_id`.

# Document Content (TODO)

+ Interactions with s3 are slow (100s of milliseconds) so the plan is to cache most documents in redis. Persist
  documents in redis right away, after they're backed up to s3 set their key to be volatile. Once ram is running out
  redis will evict these keys. Don't store documents that are larger than 250 MB in redis.
+ This is the sort of move that allows us to use our spare memory capacity, and for 90% of users result in 90% speedups
  90% of the time.