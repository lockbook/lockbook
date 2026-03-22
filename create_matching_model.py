# create_matching_model.py
import numpy as np
import onnx
from onnx import helper, TensorProto, numpy_helper
import os

print("Creating ONNX model that matches the MiniLM tokenizer...")

# Create input and output definitions (matching the tokenizer's expectations)
input_ids = helper.make_tensor_value_info(
    'input_ids', 
    TensorProto.INT64, 
    ['batch', 'seq_len']
)

attention_mask = helper.make_tensor_value_info(
    'attention_mask', 
    TensorProto.INT64, 
    ['batch', 'seq_len']
)

# Output shape: [batch, seq_len, 384] (MiniLM hidden size)
output = helper.make_tensor_value_info(
    'last_hidden_state', 
    TensorProto.FLOAT, 
    ['batch', 'seq_len', 384]  # MiniLM uses 384, not 768
)

# Create a constant node that returns zeros of the right shape
constant_value = numpy_helper.from_array(
    np.zeros((1, 128, 384), dtype=np.float32), 
    name='constant_value'
)

constant_node = helper.make_node(
    'Constant',
    inputs=[],
    outputs=['last_hidden_state'],
    value=constant_value
)

# Create graph
graph = helper.make_graph(
    [constant_node],
    'minilm_model',
    [input_ids, attention_mask],
    [output]
)

# Create model with opset 12
model = helper.make_model(
    graph,
    producer_name='minilm',
    opset_imports=[helper.make_opsetid("", 12)]
)

model.ir_version = 7

# Save model
os.makedirs("clients/models/test-model", exist_ok=True)
onnx.save(model, "clients/models/test-model/model.onnx")

print("✓ Model created successfully!")
print(f"  Location: clients/models/test-model/model.onnx")
print(f"  Size: {os.path.getsize('clients/models/test-model/model.onnx')} bytes")
print(f"  Hidden size: 384 (MiniLM compatible)")