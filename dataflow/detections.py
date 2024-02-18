import re, json, functools, ipaddress
from fnmatch import fnmatch
import logging
import apache_beam as beam
from apache_beam.options.pipeline_options import PipelineOptions





def process(batch):
    processed_batch = []
    for element in batch:
        funcs(element)
        processed_batch.append(element)

    return processed_batch


def run(argv=None):
    project = "neon-circle-400322"
    subscription = "tmp"
    subscription_str = f"projects/{project}/subscriptions/{subscription}"
    topic = f"projects/{project}/topics/topic-out-sub"

    options = PipelineOptions(argv)

    with beam.Pipeline(options=options) as p:
        input_data = (
                p
                | 'ReadFromPubSub' >> beam.io.ReadFromPubSub(subscription=subscription_str)
                | 'ProcessBatch' >> beam.ParDo(process)
        )
        input_data | 'WriteOutput' >> beam.io.WriteToPubSub(topic=topic)


if __name__ == '__main__':
    logging.getLogger().setLevel(logging.INFO)
    run()
