# File Storage
## Current Solution
Currently using DigitalOcean Spaces, their S3 equivalent, service.

Located here: [https://locked.nyc3.digitaloceanspaces.com/](https://locked.nyc3.digitaloceanspaces.com/)

## Setup
Get DigitalOcean [s3cmd](https://www.digitalocean.com/docs/spaces/resources/s3cmd/)
```shell script
s3cmd --configure
```
Set `access_key` and `secret_key` to the corresponding values from secrets

Set `host_base` to `nyc3.digitaloceanspaces.com`

Set `host_bucket` to `%(bucket).nyc3.digitaloceanspaces.com`


## Content Policy
This is the current content policy that defines how the bucket behaves.
Follows the convention of [Amazon S3 IAM policies](https://docs.aws.amazon.com/cli/latest/reference/iam/create-policy.html).
TLDR, it allows reading of every object to the public.
```json
{
  "Version":"2012-10-17",
  "Statement":[
    {
      "Sid":"PublicRead",
      "Effect":"Allow",
      "Principal": "*",
      "Action":["s3:GetObject"],
      "Resource":["arn:aws:s3:::locked/*"]
    }
  ]
}
```

To set the content policy run
```shell script
s3cmd setpolicy s3_policy.txt s3://locked
```