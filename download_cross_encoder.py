import os
import torch
from transformers import AutoTokenizer, AutoModelForSequenceClassification

print("Downloading Cross-Encoder model...")
model_name = "cross-encoder/ms-marco-MiniLM-L-6-v2"

# Create directory
os.makedirs("clients/models/cross-encoder", exist_ok=True)

# Download tokenizer
print("Downloading tokenizer...")
tokenizer = AutoTokenizer.from_pretrained(model_name)
tokenizer.save_pretrained("clients/models/cross-encoder")
print("✓ Tokenizer saved")

# Download model
print("Downloading model (this may take a few minutes)...")
model = AutoModelForSequenceClassification.from_pretrained(model_name)
model.eval()
print("✓ Model downloaded")

# Convert to ONNX
print("Converting to ONNX...")

# Create dummy inputs for cross-encoder (takes query + passage)
dummy_query = "example query"
dummy_passage = "example passage"
dummy_inputs = tokenizer(
    dummy_query, 
    dummy_passage, 
    return_tensors="pt", 
    truncation=True, 
    max_length=128
)

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

print(f"✓ Cross-encoder saved! Size: {os.path.getsize('clients/models/cross-encoder/model.onnx') / 1024 / 1024:.2f} MB")

# Verify model loads
print("\nVerifying cross-encoder...")
import onnxruntime as ort
session = ort.InferenceSession("clients/models/cross-encoder/model.onnx")
print("✓ Cross-encoder loads successfully!")
print(f"  Inputs: {[x.name for x in session.get_inputs()]}")
print(f"  Outputs: {[x.name for x in session.get_outputs()]}")

print("\n✅ Cross-encoder model ready!")