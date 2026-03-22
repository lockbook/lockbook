# test_search_system.py - Updated with cross-encoder verification
import os
import sys
import json
import numpy as np
from pathlib import Path

print("=" * 60)
print("SEARCH SYSTEM DIAGNOSTIC TEST")
print("=" * 60)

# Test 1: Check model files
print("\n[1/7] Checking Model Files...")
models_to_check = [
    ("clients/models/e5-base-v2/model.onnx", "E5 Bi-encoder", 1000),  # Should be >1GB
    ("clients/models/e5-base-v2/tokenizer.json", "E5 Tokenizer", 0.5),
    ("clients/models/cross-encoder/model.onnx", "Cross-encoder", 400),  # Should be ~400MB
    ("clients/models/cross-encoder/tokenizer.json", "Cross-encoder Tokenizer", 0.5),
]

for path, name, min_size_mb in models_to_check:
    if os.path.exists(path):
        size_mb = os.path.getsize(path) / (1024 * 1024)
        status = "✓" if size_mb > min_size_mb else "⚠"
        print(f"{status} {name}: {size_mb:.2f} MB", end="")
        if size_mb < min_size_mb:
            print(f" (WARNING: Should be >{min_size_mb} MB)")
        else:
            print()
    else:
        print(f"✗ {name}: NOT FOUND")

# Test 2: ONNX Runtime
print("\n[2/7] Testing ONNX Runtime...")
import onnxruntime as ort
print(f"✓ ONNX Runtime {ort.__version__}")
providers = ort.get_available_providers()
if 'CUDAExecutionProvider' in providers:
    print("✓ CUDA GPU acceleration available")
else:
    print("⚠ Using CPU (slower)")

# Test 3: Load and test E5 model
print("\n[3/7] Testing E5 Model...")
from transformers import AutoTokenizer
import numpy as np

e5_tokenizer = AutoTokenizer.from_pretrained("intfloat/e5-base-v2")
e5_session = ort.InferenceSession("clients/models/e5-base-v2/model.onnx")

# Test E5 inference
test_text = "passage: This is a test document"
tokens = e5_tokenizer(test_text, return_tensors="np", truncation=True, max_length=128)
outputs = e5_session.run(None, {
    'input_ids': tokens['input_ids'],
    'attention_mask': tokens['attention_mask']
})
print(f"✓ E5 model loaded and working")
print(f"  Output shape: {outputs[0].shape}")

# Test 4: Load and test Cross-Encoder
print("\n[4/7] Testing Cross-Encoder Model...")
try:
    # Load cross-encoder tokenizer
    ce_tokenizer = AutoTokenizer.from_pretrained("clients/models/cross-encoder")
    ce_session = ort.InferenceSession("clients/models/cross-encoder/model.onnx")
    
    # Check output shape
    output_shape = ce_session.get_outputs()[0].shape
    print(f"✓ Cross-encoder loaded")
    print(f"  Output shape: {output_shape}")
    
    # Test with actual query-passage pair
    test_query = "machine learning"
    test_passage = "Artificial intelligence and machine learning are related fields"
    
    inputs = ce_tokenizer(
        test_query, 
        test_passage, 
        return_tensors="np", 
        truncation=True, 
        max_length=128,
        padding="max_length"
    )
    
    outputs = ce_session.run(None, {
        'input_ids': inputs['input_ids'],
        'attention_mask': inputs['attention_mask']
    })
    
    # Apply sigmoid for probability score
    logit = outputs[0][0][0]
    score = 1.0 / (1.0 + np.exp(-logit))
    print(f"  Test query: '{test_query}'")
    print(f"  Test passage: '{test_passage[:50]}...'")
    print(f"  Relevance score: {score:.4f}")
    
    ce_working = True
except Exception as e:
    print(f"✗ Cross-encoder test failed: {e}")
    ce_working = False

# Test 5: Embedding quality
print("\n[5/7] Testing Embedding Quality...")

def get_embedding(text, is_query=False):
    prefix = "query: " if is_query else "passage: "
    tokens = e5_tokenizer(prefix + text, return_tensors="np", truncation=True, max_length=128)
    outputs = e5_session.run(None, {
        'input_ids': tokens['input_ids'],
        'attention_mask': tokens['attention_mask']
    })
    emb = outputs[0][0]
    mask = tokens['attention_mask'][0]
    pooled = np.sum(emb * mask[:, np.newaxis], axis=0) / np.sum(mask)
    norm = np.linalg.norm(pooled)
    return pooled / norm if norm > 0 else pooled

# Test documents
documents = [
    "machine learning is a subset of artificial intelligence",
    "neural networks use layers of neurons to learn patterns",
    "the weather today is sunny and warm",
    "deep learning requires large amounts of data"
]

print("Generating embeddings...")
doc_embeddings = [get_embedding(doc) for doc in documents]

# Test query
test_query = "artificial intelligence"
query_emb = get_embedding(test_query, is_query=True)

# Calculate similarities
similarities = [np.dot(query_emb, doc_emb) for doc_emb in doc_embeddings]
print(f"\nQuery: '{test_query}'")
print("Top results:")
sorted_indices = np.argsort(similarities)[::-1]
for i in sorted_indices[:3]:
    print(f"  {similarities[i]:.4f} - {documents[i]}")

# Test 6: Cross-encoder reranking
print("\n[6/7] Testing Cross-Encoder Reranking...")
if ce_working:
    # Get top 3 candidates from bi-encoder
    candidates = [(documents[i], similarities[i]) for i in sorted_indices[:5]]
    
    print("Reranking with cross-encoder...")
    reranked = []
    for doc, dense_score in candidates:
        inputs = ce_tokenizer(
            test_query,
            doc,
            return_tensors="np",
            truncation=True,
            max_length=128,
            padding="max_length"
        )
        outputs = ce_session.run(None, {
            'input_ids': inputs['input_ids'],
            'attention_mask': inputs['attention_mask']
        })
        ce_score = 1.0 / (1.0 + np.exp(-outputs[0][0][0]))
        reranked.append((doc, ce_score, dense_score))
    
    reranked.sort(key=lambda x: x[1], reverse=True)
    print("\nAfter reranking:")
    for doc, ce_score, dense_score in reranked[:3]:
        print(f"  CE:{ce_score:.4f} (dense:{dense_score:.4f}) - {doc[:50]}...")
else:
    print("⚠ Cross-encoder not available, skipping reranking test")

# Test 7: Recommendations
print("\n[7/7] Recommendations")
print("=" * 60)

print("\n✅ Your system is ready!" if ce_working else "⚠ Cross-encoder needs to be downloaded")

print("\nTo get better search results:")
print("  1. Add more content to your files (500+ chars each)")
print("  2. Use specific queries that match your content")
print("  3. Try searching with meaningful terms:")
print("     cargo run --release --bin lockbook -- search \"machine learning\"")
print("     cargo run --release --bin lockbook -- search \"neural networks\"")
print("     cargo run --release --bin lockbook -- search \"deep learning\"")

if not ce_working:
    print("\nTo enable cross-encoder reranking:")
    print("  python download_real_cross_encoder.py")
    print("  (This will download the real ~400MB model)")

print("\n" + "=" * 60)
print("TEST COMPLETE")
print("=" * 60)