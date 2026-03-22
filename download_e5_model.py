import torch
import os
from transformers import AutoTokenizer, AutoModel

print("Downloading E5-base-v2 model...")
model_name = "intfloat/e5-base-v2"

# Create directory
os.makedirs("clients/models/e5-base-v2", exist_ok=True)

# Download tokenizer
print("Downloading tokenizer...")
tokenizer = AutoTokenizer.from_pretrained(model_name)
tokenizer.save_pretrained("clients/models/e5-base-v2")
print("✓ Tokenizer saved")

# Download model
print("Downloading model (this may take a few minutes)...")
model = AutoModel.from_pretrained(model_name)
model.eval()
print("✓ Model downloaded")

# Convert to ONNX
print("Converting to ONNX (this may take a few minutes)...")

# Create dummy inputs
dummy_input_ids = torch.randint(0, 30000, (1, 128), dtype=torch.long)
dummy_attention_mask = torch.ones((1, 128), dtype=torch.long)

# Export with opset 12 for compatibility
torch.onnx.export(
    model,
    (dummy_input_ids, dummy_attention_mask),
    "clients/models/e5-base-v2/model.onnx",
    input_names=['input_ids', 'attention_mask'],
    output_names=['last_hidden_state'],
    dynamic_axes={
        'input_ids': {0: 'batch_size', 1: 'sequence_length'},
        'attention_mask': {0: 'batch_size', 1: 'sequence_length'},
        'last_hidden_state': {0: 'batch_size', 1: 'sequence_length'}
    },
    opset_version=12,
    do_constant_folding=True,
    export_params=True
)

print(f"✓ Model saved! Size: {os.path.getsize('clients/models/e5-base-v2/model.onnx') / 1024 / 1024:.2f} MB")
print("\nE5 model ready! Update your Rust code to use:")
print("  bi_model_path = \"clients/models/e5-base-v2/model.onnx\"")
print("  bi_tokenizer_path = \"clients/models/e5-base-v2/tokenizer.json\"")
print("  BI_HIDDEN: usize = 768")