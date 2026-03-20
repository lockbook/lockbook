"""
debug_onnx.py

Runs the e5-base-v2 model with a real input and prints
exactly what comes out — shape, dtype, first few values.
This tells us how to index it correctly in Rust.
"""

import numpy as np
import onnxruntime as ort

session = ort.InferenceSession("models/e5-base-v2/model.onnx")

# simulate exactly what the Rust code sends:
# batch=1, seq_len=128, all padded except first 5 tokens
seq_len  = 128
real_len = 5   # pretend we have 5 real tokens

ids   = np.zeros((1, seq_len), dtype=np.int64)
mask  = np.zeros((1, seq_len), dtype=np.int64)
types = np.zeros((1, seq_len), dtype=np.int64)

# fill first real_len positions
ids[0, :real_len]   = [101, 7592, 1010, 3086, 102]  # [CLS] hello , world [SEP]
mask[0, :real_len]  = 1

outputs = session.run(None, {
    "input_ids"      : ids,
    "attention_mask" : mask,
    "token_type_ids" : types,
})

out = outputs[0]
print(f"Output shape : {out.shape}")
print(f"Output dtype : {out.dtype}")
print(f"Total elements: {out.size}")
print(f"Expected for (1, 128, 768): {1 * 128 * 768}")
print(f"Shapes match: {out.shape == (1, 128, 768)}")
print()

# show how to correctly read token i, dimension j
# in a flat array from shape (1, seq_len, hidden)
batch, seq, hidden = out.shape
print(f"batch={batch}, seq={seq}, hidden={hidden}")
print()

# flat index for token i=0, dim j=0:
# flat_idx = batch_idx * seq * hidden + token_idx * hidden + dim_idx
# for batch_idx=0: flat_idx = token_idx * hidden + dim_idx
print("Flat index formula: token_idx * hidden + dim_idx")
print(f"Token 0, Dim 0 via shape index : {out[0, 0, 0]:.6f}")
print(f"Token 0, Dim 0 via flat index  : {out.flatten()[0 * hidden + 0]:.6f}")
print(f"Match: {out[0,0,0] == out.flatten()[0 * hidden + 0]}")
print()

# verify mean pooling result
pooled = out[0, :real_len, :].mean(axis=0)
print(f"Mean pooled shape: {pooled.shape}")
print(f"First 5 values   : {pooled[:5]}")

# verify L2 norm
norm = np.linalg.norm(pooled)
normalized = pooled / norm
print(f"After L2 norm, first 5 values: {normalized[:5]}")
print(f"Norm of normalized (should be 1.0): {np.linalg.norm(normalized):.6f}")
