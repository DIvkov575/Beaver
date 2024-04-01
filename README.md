# beaver

SIEM (data security log analysis tool)
inspired by Matano, executes completely within gcp

----

**Todo:**
-- add batching to vector output - ?why?
https://vector.dev/docs/reference/configuration/sinks/gcp_pubsub/

-- logging
-- minimize gcloud stdout + feature for verbose execution
-- `thiserror` for errors

**full list:** 
- create template + upload dataflow 
- execute template
- linking vector to detections

**bugs**
- problem with reading from resources.yaml file
- limited loop w/ name creation



**left off on**
dataflow knows functions defition (gcp_acess_policy_deleted) only if defined in record ...
how to ensure dataflwo knows hte defintion w/o needing to redefine it everytime

record has no attribute get (gcp_acess_policy_deleted)
