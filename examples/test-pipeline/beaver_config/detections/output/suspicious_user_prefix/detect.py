import re, json, functools, ipaddress
from fnmatch import fnmatch


def detect(record):
    return record.get("user") and record.get("user").startswith("temp_")
