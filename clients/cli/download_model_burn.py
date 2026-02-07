#!/usr/bin/env python3
import os
from transformers import AutoModel, AutoTokenizer, AutoConfig
import json

MODEL_NAME = "sentence-transformers/all-MiniLM-L6-v2"
OUTPUT_DIR = os.path.join(os.path.dirname(__file__), "models", "all-MiniLM-L6-v2")

os.makedirs(OUTPUT_DIR, exist_ok=True)

print("Downloading model...")
model = AutoModel.from_pretrained(MODEL_NAME)
tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
config = AutoConfig.from_pretrained(MODEL_NAME)

print("Saving files...")
tokenizer.save_pretrained(OUTPUT_DIR)
model.save_pretrained(OUTPUT_DIR, safe_serialization=True)

# Save as bert_config.json (what the library expects)
with open(f"{OUTPUT_DIR}/bert_config.json", "w") as f:
    json.dump(config.to_dict(), f, indent=2)

# Rename to bert_model.safetensors (what the library expects)
import shutil
from pathlib import Path
safetensors = list(Path(OUTPUT_DIR).glob("*.safetensors"))[0]
shutil.copy(safetensors, f"{OUTPUT_DIR}/bert_model.safetensors")

print("✅ Done!")