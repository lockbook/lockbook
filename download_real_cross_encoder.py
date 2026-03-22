# download_real_cross_encoder.py
import os
import torch
from transformers import AutoTokenizer, AutoModelForSequenceClassification

print("Downloading REAL Cross-Encoder model...")
model_name = "cross-encoder/ms-marco-MiniLM-L-6-v2"

# Create directory
os.makedirs("clients/models/cross-encoder", exist_ok=True)

# Download tokenizer
print("Downloading tokenizer...")
tokenizer = AutoTokenizer.from_pretrained(model_name)
tokenizer.save_pretrained("clients/models/cross-encoder")
print("✓ Tokenizer saved")

# Download model (this will be ~400MB)
print("Downloading model (this will take a few minutes)...")
model = AutoModelForSequenceClassification.from_pretrained(model_name)
model.eval()
print("✓ Model downloaded")

# Convert to ONNX
print("Converting to ONNX...")

# Create dummy input (query + passage)
dummy_query = "machine learning"
dummy_passage = "artificial intelligence and machine learning"
dummy_inputs = tokenizer(
    dummy_query,
    dummy_passage,
    return_tensors="pt",
    truncation=True,
    max_length=128,
    padding="max_length"
)

# Export with opset 12
torch.onnx.export(
    model,
    (dummy_inputs['input_ids'], dummy_inputs['attention_mask']),
    "clients/models/cross-encoder/model.onnx",
    input_names=['input_ids', 'attention_mask'],
    output_names=['logits'],
    dynamic_axes={
        'input_ids': {0: 'batch_size', 1: 'sequence_length'},
        'attention_mask': {0: 'batch_size', 1: 'sequence_length'},
        'logits': {0: 'batch_size'}
    },
    opset_version=12,
    do_constant_folding=True
)

print(f"✓ Model saved! Size: {os.path.getsize('clients/models/cross-encoder/model.onnx') / 1024 / 1024:.2f} MB")

# Verify
print("\nVerifying model...")
import onnxruntime as ort
session = ort.InferenceSession("clients/models/cross-encoder/model.onnx")
print("✓ Model loads successfully")
print(f"  Output shape: {session.get_outputs()[0].shape}")
print(f"  Output type: {session.get_outputs()[0].type}")

# Test inference
inputs = tokenizer(
    dummy_query,
    dummy_passage,
    return_tensors="np",
    truncation=True,
    max_length=128
)
outputs = session.run(None, {
    'input_ids': inputs['input_ids'],
    'attention_mask': inputs['attention_mask']
})
print(f"  Sample score: {outputs[0][0][0]:.4f}")

print("\n✅ Cross-encoder ready!")