"""
diagnose.py

Tests each component of the RAG pipeline to find what's crashing.
Run from your project root where models/ folder is located.

Usage:
    python diagnose.py
"""

import numpy as np
import onnxruntime as ort
from pathlib import Path

BI_MODEL   = "models/e5-base-v2/model.onnx"
BI_TOK     = "models/e5-base-v2/tokenizer.json"
CE_MODEL   = "models/cross-encoder-ms-marco-MiniLM-L-6-v2/model.onnx"
CE_TOK     = "models/cross-encoder-ms-marco-MiniLM-L-6-v2/tokenizer.json"

# ── Test 1: model files exist ─────────────────────────────────────────────────
print("\n[TEST 1] Checking model files...")
for p in [BI_MODEL, BI_TOK, CE_MODEL, CE_TOK]:
    exists = Path(p).exists()
    size   = Path(p).stat().st_size // 1024 if exists else 0
    print(f"  {'OK' if exists else 'MISSING'} {p}  ({size} KB)")

# ── Test 2: load sessions ─────────────────────────────────────────────────────
print("\n[TEST 2] Loading ONNX sessions...")
bi_session = ort.InferenceSession(BI_MODEL)
print("  OK bi-encoder loaded")
ce_session = ort.InferenceSession(CE_MODEL)
print("  OK cross-encoder loaded")

# ── Test 3: tokenizer ─────────────────────────────────────────────────────────
print("\n[TEST 3] Loading tokenizers...")
from tokenizers import Tokenizer
bi_tok = Tokenizer.from_file(BI_TOK)
ce_tok = Tokenizer.from_file(CE_TOK)
print("  OK both tokenizers loaded")

# ── Test 4: embed a simple ASCII query ───────────────────────────────────────
print("\n[TEST 4] Embedding simple ASCII query...")
MAX_LEN = 128
BI_HIDDEN = 768

def embed(session, tokenizer, text, prefix):
    prefixed = prefix + text
    enc = tokenizer.encode(prefixed)
    ids   = enc.ids
    mask  = enc.attention_mask
    types = enc.type_ids

    padded_ids   = np.zeros((1, MAX_LEN), dtype=np.int64)
    padded_mask  = np.zeros((1, MAX_LEN), dtype=np.int64)
    padded_types = np.zeros((1, MAX_LEN), dtype=np.int64)

    length = min(len(ids), MAX_LEN)
    padded_ids[0,   :length] = ids[:length]
    padded_mask[0,  :length] = mask[:length]
    padded_types[0, :length] = types[:length]

    outputs = session.run(None, {
        "input_ids"      : padded_ids,
        "attention_mask" : padded_mask,
        "token_type_ids" : padded_types,
    })
    out = outputs[0]  # shape (1, 128, 768)
    # mean pool
    pooled = out[0, :length, :].mean(axis=0)
    norm   = np.linalg.norm(pooled)
    return pooled / norm if norm > 0 else pooled

q_emb = embed(bi_session, bi_tok, "something", "query: ")
print(f"  OK query embedding shape={q_emb.shape}  norm={np.linalg.norm(q_emb):.4f}")

# ── Test 5: embed ASCII passage ───────────────────────────────────────────────
print("\n[TEST 5] Embedding ASCII passage...")
ascii_passage = "This is a simple test passage about retrieval augmented generation."
p_emb = embed(bi_session, bi_tok, ascii_passage, "passage: ")
sim = float(np.dot(q_emb, p_emb))
print(f"  OK passage embedding  sim={sim:.4f}")

# ── Test 6: embed Arabic/Unicode text ─────────────────────────────────────────
print("\n[TEST 6] Embedding Arabic/Unicode text...")
arabic_text = "هذا نص تجريبي باللغة العربية للاختبار"
try:
    a_emb = embed(bi_session, bi_tok, arabic_text, "passage: ")
    print(f"  OK Arabic embedding  norm={np.linalg.norm(a_emb):.4f}")
except Exception as e:
    print(f"  FAIL Arabic embedding: {e}")

# ── Test 7: chunk text the same way Rust does ─────────────────────────────────
print("\n[TEST 7] Chunking Unicode text...")

def chunk_text(text, size=256, overlap=51):
    chars = list(text.strip())
    total = len(chars)
    if total <= size:
        return ["".join(chars)]
    chunks = []
    start  = 0
    while start < total:
        end = min(start + size, total)
        # try sentence boundary
        search_from = max(0, end - size // 5)
        break_pos = None
        for i in range(end - 2, search_from - 1, -1):
            if chars[i] == '.' and chars[i+1] == ' ':
                break_pos = i + 1
                break
        end   = break_pos if break_pos else end
        chunk = "".join(chars[start:end]).strip()
        if chunk:
            chunks.append(chunk)
        if end >= total:
            break
        start = max(0, end - overlap)
    return chunks

test_texts = [
    ("ASCII",   "Hello world. This is a test. " * 20),
    ("Arabic",  "هذا نص تجريبي. " * 30),
    ("French",  "Voici un texte en français. Été, hiver, été. " * 20),
    ("Mixed",   "Hello مرحبا hello. Test نص test. " * 20),
    ("Emoji",   "Hello 😀 world 🌍. Test 🔥 emoji 💯. " * 20),
]

for name, text in test_texts:
    try:
        chunks = chunk_text(text)
        print(f"  OK {name:8s}  {len(chunks)} chunks  first_len={len(chunks[0]) if chunks else 0}")
    except Exception as e:
        print(f"  FAIL {name}: {e}")

# ── Test 8: embed all chunks from a Unicode text ──────────────────────────────
print("\n[TEST 8] Embedding chunks from Arabic text...")
arabic_doc = "هذا نص تجريبي باللغة العربية. " * 50
chunks = chunk_text(arabic_doc)
print(f"  {len(chunks)} chunks to embed")
for i, chunk in enumerate(chunks):
    try:
        emb = embed(bi_session, bi_tok, chunk, "passage: ")
        print(f"  OK chunk {i}  len={len(chunk)}  norm={np.linalg.norm(emb):.4f}")
    except Exception as e:
        print(f"  FAIL chunk {i}: {e}")

# ── Test 9: cross-encoder ─────────────────────────────────────────────────────
print("\n[TEST 9] Cross-encoder scoring...")

def cross_encode(session, tokenizer, query, passage):
    enc = tokenizer.encode(query, passage)
    ids   = enc.ids
    mask  = enc.attention_mask
    types = enc.type_ids

    padded_ids   = np.zeros((1, MAX_LEN), dtype=np.int64)
    padded_mask  = np.zeros((1, MAX_LEN), dtype=np.int64)
    padded_types = np.zeros((1, MAX_LEN), dtype=np.int64)

    length = min(len(ids), MAX_LEN)
    padded_ids[0,   :length] = ids[:length]
    padded_mask[0,  :length] = mask[:length]
    padded_types[0, :length] = types[:length]

    outputs = session.run(None, {
        "input_ids"      : padded_ids,
        "attention_mask" : padded_mask,
        "token_type_ids" : padded_types,
    })
    return float(outputs[0][0, 0])

score = cross_encode(ce_session, ce_tok, "something", ascii_passage)
print(f"  OK cross-encoder score={score:.4f}  sigmoid={1/(1+np.exp(-score)):.4f}")

# ── Test 10: read and process your actual files ────────────────────────────────
print("\n[TEST 10] Processing your actual .md/.txt files...")
import os

doc_dir = "."  # change this if your files are elsewhere
md_files = []
for root, dirs, files in os.walk(doc_dir):
    for f in files:
        if f.endswith(".md") or f.endswith(".txt"):
            md_files.append(os.path.join(root, f))
    if len(md_files) >= 5:
        break

if not md_files:
    print("  No .md or .txt files found in current directory")
else:
    for fpath in md_files[:5]:
        try:
            with open(fpath, encoding="utf-8", errors="replace") as f:
                content = f.read()
            chunks = chunk_text(content)
            print(f"  OK {fpath}  len={len(content)}  chunks={len(chunks)}")
            # try embedding first chunk
            emb = embed(bi_session, bi_tok, chunks[0], "passage: ")
            print(f"     first chunk embedding OK  norm={np.linalg.norm(emb):.4f}")
        except Exception as e:
            print(f"  FAIL {fpath}: {e}")

print("\n[ALL TESTS DONE]")
print("Share the output and we will fix the Rust code based on what failed.")
