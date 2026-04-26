#!/usr/bin/env python3
"""
Create a dummy ONNX vocal detection model for testing.

Generates a minimal ONNX model with:
- Input: "input" shape [1, 320] (5 frames × 64 features)
- Output: "output" shape [1, 1] (vocal probability after sigmoid)
- Single linear layer with zero weights -> constant output of sigmoid(0) = 0.5

Usage:
    python3 crates/math-audio/math-dsp/ml/create_dummy_vocal_model.py

Output:
    crates/sotf-plugins/test_data/dummy_vocal_detector.onnx
"""

import numpy as np
import os

try:
    import onnx
    from onnx import TensorProto, helper
except ImportError:
    print("Error: onnx package required. Install with: pip install onnx")
    raise SystemExit(1)

FEATURE_SIZE = 320  # 5 frames × (20 MFCCs + 20 deltas + 24 spatial/spectral)
OUTPUT_DIR = os.path.join(
    os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..")),
    "crates", "sotf-plugins", "test_data"
)
OUTPUT_PATH = os.path.join(OUTPUT_DIR, "dummy_vocal_detector.onnx")


def create_dummy_model():
    # Input: [1, 320]
    X = helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, FEATURE_SIZE])

    # Output: [1, 1]
    Y = helper.make_tensor_value_info("output", TensorProto.FLOAT, [1, 1])

    # Linear layer weights: [320, 1] all zeros
    W = helper.make_tensor(
        "W",
        TensorProto.FLOAT,
        [FEATURE_SIZE, 1],
        np.zeros(FEATURE_SIZE, dtype=np.float32).tolist(),
    )

    # Bias: [1] zero
    B = helper.make_tensor(
        "B",
        TensorProto.FLOAT,
        [1],
        [0.0],
    )

    # MatMul + Add = Linear layer
    matmul_node = helper.make_node("MatMul", ["input", "W"], ["matmul_out"])
    add_node = helper.make_node("Add", ["matmul_out", "B"], ["linear_out"])
    sigmoid_node = helper.make_node("Sigmoid", ["linear_out"], ["output"])

    graph = helper.make_graph(
        [matmul_node, add_node, sigmoid_node],
        "dummy_vocal_detector",
        [X],
        [Y],
        initializer=[W, B],
    )

    model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 13)])
    model.ir_version = 8

    # Validate
    onnx.checker.check_model(model)

    # Save
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    onnx.save(model, OUTPUT_PATH)
    print(f"Saved dummy model to: {OUTPUT_PATH}")
    print(f"  Input:  'input'  shape [1, {FEATURE_SIZE}]")
    print(f"  Output: 'output' shape [1, 1]")
    print(f"  Expected output: sigmoid(0) = 0.5 for any input")


if __name__ == "__main__":
    create_dummy_model()
