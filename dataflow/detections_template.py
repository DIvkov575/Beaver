"""Beaver SIEM Dataflow entrypoint.

Delegates to `sigma_beam.correlation_pipeline.run()`. The Rust deploy code
ships this file as the Dataflow template body and passes the runtime args
(--input_subscription, --alerts_topic, --dlq_topic, --rules_uri, etc.) on
the gcloud command line.

The old per-rule AST-spliced detection code is gone — sigma_beam loads
the rules from GCS at worker startup instead.
"""
import logging

from sigma_beam.correlation_pipeline import run

if __name__ == "__main__":
    logging.getLogger().setLevel(logging.INFO)
    run()
