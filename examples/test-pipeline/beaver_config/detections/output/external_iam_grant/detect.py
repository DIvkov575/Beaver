import re, json, functools, ipaddress
from fnmatch import fnmatch


def detect(record):
    return (
        record.get("event") == "iam_change"
        and record.get("action") == "grant"
        and not (record.get("granted_to") and record.get("granted_to").endswith("@neon-circle.io"))
    )
