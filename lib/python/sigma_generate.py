import argparse

from sigma.backends.matano import MatanoPythonBackend
from sigma.collection import SigmaCollection


def main():
    parser = argparse.ArgumentParser(description="pySigma Matano backend. See https://www.matano.dev/docs/detections/importing-from-sigma-rules for usage instructions.")
    parser.add_argument("filepath", type=str, help="File path to Sigma rule to convert.")
    args = parser.parse_args()

    filepath = args.filepath
    pipeline_name = args.pipeline
    pipeline = None

    backend = MatanoPythonBackend(processing_pipeline=pipeline)

    with open(filepath) as f:
        ruleraw = f.read()
    rule = SigmaCollection.from_yaml(ruleraw)
    backend.convert(rule, "detection")


if __name__ == "__main__":
    main()
