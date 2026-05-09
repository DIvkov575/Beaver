import re, json, functools, ipaddress
from fnmatch import fnmatch
import logging
import apache_beam as beam
from apache_beam.options.pipeline_options import PipelineOptions, GoogleCloudOptions


class DetectionsOptions(PipelineOptions):
    @classmethod
    def _add_argparse_args(cls, parser):
        parser.add_argument(
            '--subscription',
            required=True,
            help='Pub/Sub subscription id (just the id; project comes from --project)',
        )


def run(argv=None):
    def harness(test):
        def detection(record, /):
            if test(record):
                # Structured payload — Cloud Logging parses this into
                # jsonPayload.rule_name etc, which alert policies match on.
                logging.warning(json.dumps({
                    "event": "BEAVER_SIEM_MATCH",
                    "rule_name": test.__name__,
                    "record": record,
                    "message": f"rule {test.__name__!r} matched",
                }))

        return detection


    def process(element):
        raw = element.data.decode("utf-8")
        try:
            record = json.loads(raw)
        except json.JSONDecodeError:
            logging.warning(json.dumps({
                "event": "BEAVER_SIEM_DROP",
                "reason": "non-JSON",
                "message": "dropping non-JSON message",
            }))
            return element
        detections(record)
        return element

    options = PipelineOptions(argv, streaming=True)
    my_options = options.view_as(DetectionsOptions)
    gc_options = options.view_as(GoogleCloudOptions)
    subscription_path = (
        f"projects/{gc_options.project}/subscriptions/{my_options.subscription}"
    )

    with beam.Pipeline(options=options) as p:
        (
                p
                | 'ReadFromPubSub' >> beam.io.ReadFromPubSub(
                    subscription=subscription_path,
                    with_attributes=True
                )
                | 'RunDetections' >> beam.Map(process)
        )


if __name__ == '__main__':
    logging.getLogger().setLevel(logging.INFO)
    run()
