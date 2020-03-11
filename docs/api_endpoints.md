# API Spec

Common semantics:
+ Responses, if any, are JSON dictionaries with fields documented per-endpoint.
+ Error reponses are JSON objects with an `error_code` field and endpoint-specific fields if any.
+ `GET` requests have parameters encoded in the url in order e.g. `GET /get-updates/my-username/my-auth/my-version`
+ non-`GET` requests have parameters in request body with content type `x-www-form-urlencoded`

Common errors:
+ `401 invalid_auth`: `auth` does not decrypt to a string of the form `${username},${unix_timestamp_millis}`
+ `401 expired_auth`: timestamp in `auth` is too old
+ `403 not_permissioned`: user could be authenticated but did not have permission to perform the action
+ `404 user_not_found`

## New Account

`POST /new-account`

Inputs:
+ `username`: SHA1-hashed username
+ `auth`: string `${username},${unix_timestamp_millis}` signed by user's private key
+ `pub_key_n`: modulus of user's RSA public key 
+ `pub_key_e`: exponent of user's RSA public key

Outputs:
+ `201`
+ `422 username_taken`

## Create File

`POST /create-file`

Inputs:
+ `username`: SHA1-hashed username
+ `auth`: string `${username},${unix_timestamp_millis}` signed by user's private key
+ `file_id`: UUID to identify file
+ `file_name`: base64-encoded encrypted file name
+ `file_path`: base64-encoded encrypted file path
+ `file_content`: base64-encoded encrypted file contents

Outputs:
+ `201`: response contains `file_version` (required for making edits)
+ `422 file_id_taken`
+ `422 file_path_taken`

## Change File Content

`PUT /change-file-content`

Inputs:
+ `username`: SHA1-hashed username
+ `auth`: string `${username},${unix_timestamp_millis}` signed by user's private key
+ `file_id`: UUID to identify file
+ `old_file_version`: the version of the file before changes are applied
+ `new_file_content`: new base64-encoded encrypted file contents

Outputs:
+ `200`: response contains `file_version` (required for making edits)
+ `404 file_not_found`
+ `409 edit_conflict`: `old_file_version` is incorrect; response contains `current_version`
+ `410 file_deleted`

## Rename File

`PUT /rename-file`

Inputs:
+ `username`: SHA1-hashed username
+ `auth`: string `${username},${unix_timestamp_millis}` signed by user's private key
+ `file_id`: UUID to identify file
+ `new_file_name`: new base64-encoded encrypted file name

Outputs:
+ `204`
+ `404 file_not_found`
+ `410 file_deleted`

## Move File

`PUT /move-file`

Inputs:
+ `username`: SHA1-hashed username
+ `auth`: string `${username},${unix_timestamp_millis}` signed by user's private key
+ `file_id`: UUID to identify file
+ `new_file_path`: new base64-encoded encrypted file path

Outputs:
+ `204`
+ `404 file_not_found`
+ `410 file_deleted`
+ `422 file_path_taken`

## Delete File

`DELETE /delete-file`

Inputs:
+ `username`: SHA1-hashed username
+ `auth`: string `${username},${unix_timestamp_millis}` signed by user's private key
+ `file_id`: UUID to identify file

Outputs:
+ `204`
+ `404 file_not_found`
+ `410 file_deleted`

## Get Updated Metadata

`GET /get-updated-metadata/<username>/<auth>/<since_version>`

Inputs:
+ `username`: SHA1-hashed username
+ `auth`: string `${username},${unix_timestamp_millis}` signed by user's private key
+ `since_version`: all updates after this version are included in response

Outputs:
+ `200`: response contains list of file ids updated since `version`

## Get File

`GET /get-file/<file_id>`

Inputs:
+ `file_id`: UUID to identify file

Outputs:
+ `200`: response contains base64-encoded encrypted `file_content`
