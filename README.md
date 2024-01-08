# beaver

SEIM (data security log analysis tool)
inspired by matano, executes completely within gcp

----



**Todo:**
-- fix cloud run vector instance on gcloud
  2024-01-07T22:22:02.424692Z ERROR vector::topology::builder: Configuration error. error=Transform "transform1": must provide exactly one of `source` or `file` configuration

-- give cron access to running cloud run
  https://cloud.google.com/scheduler/docs/http-target-auth
-- gcloud check
-- logging
-- minimize gcloud stdout + feature for verbose execution

**full list:** 
- create bq !! needs testing
  - create bq dataset
  - create bq table w/ json col
- create o_pubsub w/ auto-write to bq !! needs testing
- generate intermediate files
  - vector.yaml
    - link to pubsub
    - import from config
- bucket
  - create bucket
  - upload vec.yaml to bucket
- crj
  - create crj
  - link crj
  - update crk
- scheduler
  - create scheduler
  - run scheduler 

- 

