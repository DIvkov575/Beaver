"""Dataflow entrypoint for the sigma_beam streaming pipeline.

`generate_detections_file` copies this file to `artifacts/detections_gen.py`,
which the deploy then builds into a Classic Dataflow template. The actual
pipeline lives in `sigma_beam.correlation_pipeline`; it reads its runtime
args (`--input_subscription`, `--alerts_topic`, `--dlq_topic`, `--rules_uri`)
straight off the command line the template build passes in.
"""
import logging

from sigma_beam.correlation_pipeline import run

if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    run()
