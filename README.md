# beaver

SEIM (data security log analysis tool)
inspired by matano, executes completely within gcp

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
