import re, json, functools, ipaddress
from fnmatch import fnmatch
import logging
import apache_beam as beam
from apache_beam.options.pipeline_options import PipelineOptions



def process(element):
    funcs(element)
    logging.info(element)

    return element


class DetectionsOptions(PipelineOptions):
    @classmethod
    def _add_argparse_args(cls, parser):
        parser.add_argument('project', help='Gcloud project ID')
        parser.add_argument('subscription', help='input pubsub subscription id')


def run(argv=None):
    options = PipelineOptions(argv, streaming=True)
    my_options = options.view_as(DetectionsOptions)

    with beam.Pipeline(options=options) as p:
        (
                p
                | 'ReadFromPubSub' >> beam.io.ReadFromPubSub(
                    subscription=f"projects/{my_options.project}/subscriptions/{my_options.subscription}",
                    with_attributes=True
                )
                | 'ProcessBatch' >> beam.Map(process)
        )


if __name__ == '__main__':
    logging.getLogger().setLevel(logging.INFO)
    run()
