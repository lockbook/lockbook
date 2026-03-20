"""
benchmark.py

Run this to find exactly where the slowness comes from.
Tests each step independently with precise timing.

Usage:
    python benchmark.py
"""

import time
import numpy as np
import onnxruntime as ort

BI_MODEL   = "models/e5-base-v2/model.onnx"
BI_TOK     = "models/e5-base-v2/tokenizer.json"
BI_HIDDEN  = 768
MAX_LEN    = 128
BATCH_SIZE = 32

def ms(start): return (time.perf_counter() - start) * 1000

print("\n========================================")
print("  SEARCH BENCHMARK")
print("========================================\n")

# ── Test 1: Model load time ───────────────────────────────────────────────────
print("[1/6] Loading model...")
t = time.perf_counter()

# try GPU first
providers = ort.get_available_providers()
print(f"      Available providers: {providers}")

if "CUDAExecutionProvider" in providers:
    session = ort.InferenceSession(BI_MODEL, providers=["CUDAExecutionProvider"])
    print(f"      Using: GPU (CUDA)")
else:
    session = ort.InferenceSession(BI_MODEL, providers=["CPUExecutionProvider"])
    print(f"      Using: CPU  ← GPU NOT AVAILABLE")

print(f"      Model load time: {ms(t):.0f}ms\n")

# ── Test 2: Tokenizer ─────────────────────────────────────────────────────────
print("[2/6] Loading tokenizer...")
t = time.perf_counter()
from tokenizers import Tokenizer
tokenizer = Tokenizer.from_file(BI_TOK)
print(f"      Tokenizer load: {ms(t):.0f}ms\n")

# ── Test 3: Single inference ──────────────────────────────────────────────────
print("[3/6] Single inference (batch=1)...")
dummy = "passage: This is a test sentence for benchmarking embedding speed."
enc   = tokenizer.encode(dummy)
ids   = enc.ids[:MAX_LEN]
mask  = enc.attention_mask[:MAX_LEN]
types = enc.type_ids[:MAX_LEN]

def pad(arr, length=MAX_LEN):
    out = np.zeros((1, length), dtype=np.int64)
    out[0, :len(arr)] = arr
    return out

inputs = {
    "input_ids"      : pad(ids),
    "attention_mask" : pad(mask),
    "token_type_ids" : pad(types),
}

# warm up
session.run(None, inputs)
print(f"      Warm-up done")

# measure
times = []
for _ in range(5):
    t = time.perf_counter()
    session.run(None, inputs)
    times.append(ms(t))

single_ms = np.mean(times)
print(f"      Single call avg (5 runs): {single_ms:.1f}ms")
print(f"      Min: {min(times):.1f}ms  Max: {max(times):.1f}ms\n")

# ── Test 4: Batch of 32 ───────────────────────────────────────────────────────
print(f"[4/6] Batch inference (batch={BATCH_SIZE})...")

batch_ids   = np.zeros((BATCH_SIZE, MAX_LEN), dtype=np.int64)
batch_mask  = np.zeros((BATCH_SIZE, MAX_LEN), dtype=np.int64)
batch_types = np.zeros((BATCH_SIZE, MAX_LEN), dtype=np.int64)

for b in range(BATCH_SIZE):
    l = len(ids)
    batch_ids[b,   :l] = ids
    batch_mask[b,  :l] = mask
    batch_types[b, :l] = types

batch_inputs = {
    "input_ids"      : batch_ids,
    "attention_mask" : batch_mask,
    "token_type_ids" : batch_types,
}

times = []
for _ in range(3):
    t = time.perf_counter()
    session.run(None, batch_inputs)
    times.append(ms(t))

batch_ms = np.mean(times)
per_chunk = batch_ms / BATCH_SIZE
print(f"      Batch call avg: {batch_ms:.1f}ms")
print(f"      Per chunk:      {per_chunk:.2f}ms")
print(f"      Speedup vs single: {single_ms/per_chunk:.1f}x\n")

# ── Test 5: 50 sequential single calls ────────────────────────────────────────
print("[5/6] 50 sequential single calls (simulates old code)...")
t = time.perf_counter()
for _ in range(50):
    session.run(None, inputs)
seq_ms = ms(t)
print(f"      50 calls total: {seq_ms:.0f}ms  ({seq_ms/50:.1f}ms each)\n")

# ── Test 6: Tokenization speed ────────────────────────────────────────────────
print("[6/6] Tokenization speed (500 chunks)...")
t = time.perf_counter()
for _ in range(500):
    tokenizer.encode(dummy)
tok_ms = ms(t)
print(f"      500 tokenizations: {tok_ms:.0f}ms  ({tok_ms/500:.3f}ms each)\n")

# ── Summary ───────────────────────────────────────────────────────────────────
print("========================================")
print("  SUMMARY")
print("========================================")
print(f"  Provider:              {session.get_providers()[0]}")
print(f"  Single inference:      {single_ms:.1f}ms")
print(f"  Batch/32 per chunk:    {per_chunk:.2f}ms")
print(f"  50 sequential calls:   {seq_ms:.0f}ms")
print(f"  Tokenization each:     {tok_ms/500:.3f}ms")
print()
print(f"  For 100 chunks, your app needs:")
print(f"    Sequential (old):    {single_ms * 100:.0f}ms")
print(f"    Batched (new):       {per_chunk * 100:.0f}ms")
print("========================================\n")

if single_ms > 200:
    print("PROBLEM: >200ms per call — definitely CPU, not GPU")
    print("         Check onnxruntime_providers_cuda.dll location")
elif single_ms > 50:
    print("PROBLEM: >50ms per call — GPU not being used effectively")
elif single_ms > 20:
    print("OK-ISH:  GPU working but slower than expected for RTX 4060")
    print("         Expected: 5-15ms on RTX 4060")
else:
    print("GOOD: GPU is working correctly")
