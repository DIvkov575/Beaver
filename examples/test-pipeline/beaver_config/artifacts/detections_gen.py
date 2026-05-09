import re, json, functools, ipaddress
from fnmatch import fnmatch
import logging
import apache_beam as beam
from apache_beam.options.pipeline_options import PipelineOptions, GoogleCloudOptions

class DetectionsOptions(PipelineOptions):
    
    @classmethod
    def _add_argparse_args(cls, parser):
        parser.add_argument("--subscription", required=True, help="Pub/Sub subscription id (just the id; project comes from --project)")



def run(argv=None):
    
    def harness(test):
        
        def detection(record, /):
            if test(record):
                logging.warning(json.dumps({"event":"BEAVER_SIEM_MATCH", "rule_name":test.__name__, "record":record, "message":f"rule {test.__name__!r} matched"}))

        return detection

    
    @harness
    def failed_login(record, /):
        return (record.get("event")=="login") and (record.get("status")=="failed")

    
    @harness
    def suspicious_user_prefix(record, /):
        return record.get("user") and record.get("user").startswith("temp_")

    
    @harness
    def privileged_role_grant(record, /):
        return (record.get("event")=="iam_change")  and  (record.get("action")=="grant")  and  (record.get("role") in ("roles/owner", "roles/editor", "roles/iam.securityAdmin"))

    
    @harness
    def bulk_data_export(record, /):
        return (record.get("event")=="data_access") and (record.get("operation") in ("export", "download_bulk"))

    
    @harness
    def external_iam_grant(record, /):
        return (record.get("event")=="iam_change")  and  (record.get("action")=="grant")  and  (not (record.get("granted_to") and record.get("granted_to").endswith("@neon-circle.io")))

    
    def detections(record, /):
        failed_login(record)
        suspicious_user_prefix(record)
        privileged_role_grant(record)
        bulk_data_export(record)
        external_iam_grant(record)

    
    def process(element):
        raw = element.data.decode("utf-8")
        try:
            record = json.loads(raw)
        except json.JSONDecodeError:
            logging.warning(json.dumps({"event":"BEAVER_SIEM_DROP", "reason":"non-JSON", "message":"dropping non-JSON message"}))
            return element
        detections(record)
        return element

    options = PipelineOptions(argv, streaming=True)
    my_options = options.view_as(DetectionsOptions)
    gc_options = options.view_as(GoogleCloudOptions)
    subscription_path = f"projects/{gc_options.project}/subscriptions/{my_options.subscription}"
    with beam.Pipeline(options=options) as p:
        (p) | ("ReadFromPubSub">>beam.io.ReadFromPubSub(subscription=subscription_path, with_attributes=True)) | ("RunDetections">>beam.Map(process))

if __name__=="__main__":
    logging.getLogger().setLevel(logging.INFO)
    run()
