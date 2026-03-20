"""
check_models.py

Checks the input/output shapes of both exported ONNX models.
Run this to verify dimensions before using in Rust.

Usage:
    pip install onnxruntime numpy
    python check_models.py
"""

import numpy as np
import onnxruntime as ort
from pathlib import Path

def check_model(model_path: str, name: str):
    print(f"\n{'='*50}")
    print(f"Model: {name}")
    print(f"Path : {model_path}")

    if not Path(model_path).exists():
        print("  ERROR: model.onnx not found")
        return

    session = ort.InferenceSession(model_path)

    print("\n  Inputs:")
    for inp in session.get_inputs():
        print(f"    name={inp.name}  shape={inp.shape}  type={inp.type}")

    print("\n  Outputs:")
    for out in session.get_outputs():
        print(f"    name={out.name}  shape={out.shape}  type={out.type}")

    # run a quick test with small input
    seq_len = 32
    dummy_ids   = np.zeros((1, seq_len), dtype=np.int64)
    dummy_mask  = np.ones((1, seq_len),  dtype=np.int64)
    dummy_types = np.zeros((1, seq_len), dtype=np.int64)

    try:
        outputs = session.run(None, {
            "input_ids"      : dummy_ids,
            "attention_mask" : dummy_mask,
            "token_type_ids" : dummy_types,
        })
        print(f"\n  Test output shapes:")
        for i, out in enumerate(outputs):
            print(f"    output[{i}]: shape={out.shape}  dtype={out.dtype}")
    except Exception as e:
        print(f"\n  Test run failed: {e}")
        # try without token_type_ids
        try:
            outputs = session.run(None, {
                "input_ids"      : dummy_ids,
                "attention_mask" : dummy_mask,
            })
            print(f"\n  Test output shapes (no token_type_ids):")
            for i, out in enumerate(outputs):
                print(f"    output[{i}]: shape={out.shape}  dtype={out.dtype}")
        except Exception as e2:
            print(f"  Also failed without token_type_ids: {e2}")

check_model("models/e5-base-v2/model.onnx", "e5-base-v2 (bi-encoder)")
check_model("models/cross-encoder-ms-marco-MiniLM-L-6-v2/model.onnx",
            "ms-marco-MiniLM-L-6-v2 (cross-encoder)")

print("\n" + "="*50)
print("Done. Share these shapes to fix the Rust code.")
