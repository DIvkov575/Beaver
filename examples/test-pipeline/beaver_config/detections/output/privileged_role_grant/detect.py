import re, json, functools, ipaddress
from fnmatch import fnmatch


def detect(record):
    return (
        record.get("event") == "iam_change"
        and record.get("action") == "grant"
        and (record.get("role") in ("roles/owner", "roles/editor", "roles/iam.securityAdmin"))
    )
