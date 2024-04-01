import re, json, functools, ipaddress
from fnmatch import fnmatch
import logging
import apache_beam as beam
from apache_beam.io.gcp.pubsub import PubsubMessage
from apache_beam.options.pipeline_options import PipelineOptions


class DetectionsOptions(PipelineOptions):
    @classmethod
    def _add_argparse_args(cls, parser):
        parser.add_argument("project", help="Gcloud project ID")
        parser.add_argument("subscription", help="input pubsub subscription id")


def run(argv=None):
    def harness(test):
        def detection(record, /):
            if test(record):
                logging.warning(f"record ({str(record)}) failed test ({test}): ")

        return detection

    @harness
    def custom_test1(record, /):
        logging.info("custom_test1")
        return (record.get("hello") == "world")

    @harness
    def gcp_access_policy_deleted(record, /):
        logging.info("gcp_access_policy_deleted")
        return (record.get("data", {}).get("protoPayload", {}).get("authorizationInfo", {}).get("permission") in (
            "accesscontextmanager.accessPolicies.delete", "accesscontextmanager.accessPolicies.accessLevels.delete",
            "accesscontextmanager.accessPolicies.accessZones.delete",
            "accesscontextmanager.accessPolicies.authorizedOrgsDescs.delete")) and (
                record.get("data", {}).get("protoPayload", {}).get("authorizationInfo", {}).get(
                    "granted") == "true") and (record.get("data", {}).get("protoPayload", {}).get(
            "serviceName") == "accesscontextmanager.googleapis.com")

    def detections(record, /):
        custom_test1(record)
        gcp_access_policy_deleted(record)

    def process(element: PubsubMessage, /) -> PubsubMessage:
        data = element.data.decode("utf-8")
        detections(json.loads(data))
        return element

    options = PipelineOptions(argv, streaming=True)
    my_options = options.view_as(DetectionsOptions)
    with beam.Pipeline(options=options) as p:
        (p) | ("ReadFromPubSub" >> beam.io.ReadFromPubSub(subscription=my_options.subscription,
                                                          with_attributes=True)) | (
                "ProcessBatch" >> beam.Map(process))


logging.getLogger().setLevel(logging.INFO)
run()
