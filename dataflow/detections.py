import logging

import apache_beam as beam
from apache_beam.options.pipeline_options import PipelineOptions


def process_batch(batch):
    processed_batch = []
    for element in batch:
        logging.info("stuff")
        processed_batch.append(element)

    return processed_batch


def run(argv=None):

    project = "neon-circle-400322"
    sub1 = "tmp"
    sub2 = "tmp2"
    sub_str_1 = f"projects/{project}/subscriptions/{sub1}"
    sub_str_2 = f"projects/{project}/subscriptions/{sub2}"

    options = PipelineOptions(argv)

    with beam.Pipeline(options=options) as p:
        input_data = (
                p
                | 'ReadFromPubSub' >> beam.io.ReadFromPubSub(subscription=sub_str_1)
                | 'ProcessBatch' >> beam.ParDo(process_batch)
        )
        input_data | 'WriteOutput' >> beam.io.WriteToPubSub(subscription=sub_str_2)


if __name__ == '__main__':
    logging.getLogger().setLevel(logging.INFO)
    run()
