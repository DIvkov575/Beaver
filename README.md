# beaver

SEIM (data security log analysis tool)
inspired by matano, executes completely within gcp

----



**Todo:**
-- add batching to vector output
https://vector.dev/docs/reference/configuration/sinks/gcp_pubsub/


-- logging
-- minimize gcloud stdout + feature for verbose execution
-- `thiserror` for errors

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

