import fcntl
import gzip
import json
import os
import re
import sys
import time

directory, operation, *args = sys.argv[1:]


def identity(stat):
    return f"{stat.st_dev}-{stat.st_ino}"


def open_segment(number):
    return open(os.path.join(directory, f"db.{number}.log"), "rb", buffering=0)


def snapshot_end(file, number, length):
    if number == 0:
        return 0
    header = file.read(18)
    if len(header) < 10 or header[8] != 0:
        return 0
    tag = header[9]
    widths = {251: 2, 252: 4, 253: 8}
    width = widths.get(tag, 0)
    if len(header) < 10 + width or (width == 0 and tag >= 251):
        return 0
    sequence = int.from_bytes(header[10:10 + width], "little") if width else tag
    end = 8 + int.from_bytes(header[:8], "big")
    return end if sequence == number and 10 + width <= end <= length else 0


def describe(number):
    with open_segment(number) as file:
        stat = os.fstat(file.fileno())
        return dict(number=number, identity=identity(stat), length=stat.st_size,
                    stamp=stat.st_mtime_ns, first_end=snapshot_end(file, number, stat.st_size))


if operation == "inventory":
    with open(os.path.join(directory, "db.lock"), "rb") as lock:
        fcntl.flock(lock, fcntl.LOCK_SH)
        numbers = []
        for name in os.listdir(directory):
            match = re.fullmatch(r"db\.(0|[1-9][0-9]*)\.log", name)
            if match is not None:
                numbers.append(int(match[1]))
        numbers.sort()
        active = describe(numbers[-1]) if numbers else None
        observed = time.time_ns() // 1_000_000
    segments = []
    for number in numbers[:-1]:
        try:
            segments.append(describe(number))
        except FileNotFoundError:
            # A retired segment may still be in the admin's cache.
            pass
    if active is not None:
        segments.append(active)
    print(json.dumps(dict(segments=segments, observed=observed)))
elif operation == "read":
    number, expected, stamp, length, start, end = args
    start, end, length = int(start), int(end), int(length)
    with open_segment(int(number)) as file:
        stat = os.fstat(file.fileno())
        if (identity(stat) != expected or stat.st_size < length
                or (stat.st_size == length and stat.st_mtime_ns != int(stamp))
                or not 0 <= start <= end <= length):
            raise RuntimeError("database segment changed during refresh")
        file.seek(start)
        with gzip.GzipFile(fileobj=sys.stdout.buffer, mode="wb", compresslevel=3) as output:
            remaining = end - start
            while remaining:
                data = file.read(min(remaining, 1024 * 1024))
                if not data:
                    raise RuntimeError("database segment was shortened during refresh")
                output.write(data)
                remaining -= len(data)
else:
    raise ValueError("unknown metrics acquisition operation")
