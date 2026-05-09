import re, json, functools, ipaddress
from fnmatch import fnmatch


def detect(record):
    return record.get("event") == "data_access" and (
        record.get("operation") in ("export", "download_bulk")
    )
