# create_compatible_model.py
import numpy as np
import onnx
import onnxruntime as ort
from onnx import helper, TensorProto, numpy_helper, checker
import json
import os

# Create models directory
os.makedirs("clients/models/test-model", exist_ok=True)

print("Creating ONNX model with IR version 7 (compatible)...")

# Create input and output definitions
input_tensor = helper.make_tensor_value_info(
    'input_ids', 
    TensorProto.INT64, 
    ['batch', 'seq_len']
)

output_tensor = helper.make_tensor_value_info(
    'last_hidden_state', 
    TensorProto.FLOAT, 
    ['batch', 'seq_len', 768]
)

# Create a simple identity node
node = helper.make_node(
    'Identity',
    inputs=['input_ids'],
    outputs=['output_ids']
)

# Create a constant node for the output shape
constant_node = helper.make_node(
    'ConstantOfShape',
    inputs=['input_ids_shape'],
    outputs=['last_hidden_state'],
    value=numpy_helper.from_array(np.zeros((1,), dtype=np.float32))
)

# Get shape node to get input shape
shape_node = helper.make_node(
    'Shape',
    inputs=['input_ids'],
    outputs=['input_ids_shape']
)

# Create graph
graph = helper.make_graph(
    [shape_node, constant_node],  # nodes
    'test_model',                  # name
    [input_tensor],                # inputs
    [output_tensor]                # outputs
)

# Create model with IR version 7 (compatible with older ONNX Runtime)
model = helper.make_model(
    graph,
    producer_name='test',
    opset_imports=[helper.make_opsetid("", 12)]  # Use opset 12
)

# Set IR version to 7
model.ir_version = 7

# Check model
try:
    checker.check_model(model)
    print("✓ Model is valid")
except Exception as e:
    print(f"Model validation failed: {e}")
    # Continue anyway for testing

# Save model
onnx.save(model, 'clients/models/test-model/model.onnx')
print(f"Model saved: clients/models/test-model/model.onnx")
print(f"Size: {os.path.getsize('clients/models/test-model/model.onnx')} bytes")

# Create tokenizer.json
tokenizer_config = {
    "version": "1.0",
    "model": {
        "vocab": {"test": 0}
    }
}

with open('clients/models/test-model/tokenizer.json', 'w') as f:
    json.dump(tokenizer_config, f)

print("✓ Tokenizer saved")

# Test loading
print("\nTesting model loading...")
try:
    session = ort.InferenceSession('clients/models/test-model/model.onnx')
    print("✓ Model loads successfully!")
    
    # Test inference
    input_data = np.zeros((1, 128), dtype=np.int64)
    outputs = session.run(None, {'input_ids': input_data})
    print(f"✓ Inference works! Outputs: {len(outputs)}")
    
except Exception as e:
    print(f"✗ Failed: {e}")