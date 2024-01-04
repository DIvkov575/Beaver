# beaver

SEIM (data security log analysis tool)
inspired by matano, executes completely within gcp

----

- create bq
    - create bq dataset
    - create bq table w/ json col
- create o_pubsub w/ auto-write to bq
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

