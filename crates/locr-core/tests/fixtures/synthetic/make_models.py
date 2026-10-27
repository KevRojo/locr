"""Generate tiny *synthetic* ocrs-compatible models for offline tests.

These are NOT real OCR models. They have the same input/output contract as
the ocrs detection and recognition models, with hand-picked weights, so the
full ocrs pipeline (detection -> layout -> recognition -> decoding) can run in
environments that cannot download the real models (~12 MB, CC-BY-SA 4.0).

  detection   [1, 1, 800, 600] -> [1, 1, 800, 600]
              Dark-grey pixels are "text", pure black (ocrs' padding value)
              and white are not. A horizontal max-pool merges glyphs that are
              close together into word blobs.

  recognition [N, 1, 64, W] -> [W/4, N, 97] log-probabilities
              Each 64x4 column block is averaged and mapped to the class whose
              anchor intensity is closest (white -> CTC blank). A term from the
              mean over the whole (padded) width makes every frame depend on
              the batch padding, like the bidirectional RNN of the real model.
              The output is deterministic and depends on the pixels of the line
              image, which is what differential tests need.

Usage (from this directory):

    pip install onnx numpy rten-convert
    python make_models.py
"""

import os
import shutil
import subprocess
import sys

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

HERE = os.path.dirname(os.path.abspath(__file__))
OPSET = [helper.make_opsetid("", 17)]

DET_HEIGHT, DET_WIDTH = 800, 600
REC_HEIGHT = 64
NUM_CLASSES = 97  # CTC blank + the 96-char ocrs default alphabet


def const(name, value):
    return numpy_helper.from_array(np.asarray(value, dtype=np.float32), name)


def detection_model():
    x = helper.make_tensor_value_info("image", TensorProto.FLOAT, [1, 1, DET_HEIGHT, DET_WIDTH])
    y = helper.make_tensor_value_info("mask", TensorProto.FLOAT, [1, 1, DET_HEIGHT, DET_WIDTH])
    nodes = [
        # darkness = sigmoid(-20 x): ~1 for dark pixels, ~0 for white ones.
        helper.make_node("Mul", ["image", "neg_k"], ["dark_logit"]),
        helper.make_node("Sigmoid", ["dark_logit"], ["dark"]),
        # not_black = sigmoid(1000 (x + 0.495)): suppresses pure black padding.
        helper.make_node("Add", ["image", "black_bias"], ["shifted"]),
        helper.make_node("Mul", ["shifted", "black_k"], ["not_black_logit"]),
        helper.make_node("Sigmoid", ["not_black_logit"], ["not_black"]),
        helper.make_node("Mul", ["dark", "not_black"], ["glyph"]),
        # Merge glyphs into words.
        helper.make_node(
            "MaxPool",
            ["glyph"],
            ["mask"],
            kernel_shape=[3, 11],
            strides=[1, 1],
            pads=[1, 5, 1, 5],
        ),
    ]
    graph = helper.make_graph(
        nodes,
        "locr_synthetic_detection",
        [x],
        [y],
        initializer=[const("neg_k", -20.0), const("black_bias", 0.495), const("black_k", 1000.0)],
    )
    return helper.make_model(graph, opset_imports=OPSET)


def recognition_model():
    x = helper.make_tensor_value_info("line_images", TensorProto.FLOAT, ["batch", 1, REC_HEIGHT, "width"])
    y = helper.make_tensor_value_info("chars", TensorProto.FLOAT, ["seq", "batch", NUM_CLASSES])

    # Anchor intensity per class: blank at white, characters spread from black
    # to light grey. logit_c = k * (2 t_c m - t_c^2) picks the class whose
    # anchor t_c is closest to the column mean m.
    anchors = np.concatenate([[0.5], np.linspace(-0.5, 0.4, NUM_CLASSES - 1)]).astype(np.float32)
    k = 40.0
    weight = (k * 2.0 * anchors).reshape(NUM_CLASSES, 1, 1, 1)
    bias = -k * anchors**2
    # Padding sensitivity: per-class response to the mean over the full width.
    global_weight = (4.0 * np.sin(1.7 * np.arange(NUM_CLASSES))).astype(np.float32).reshape(NUM_CLASSES, 1, 1, 1)

    nodes = [
        helper.make_node(
            "AveragePool",
            ["line_images"],
            ["cols"],
            kernel_shape=[REC_HEIGHT, 4],
            strides=[REC_HEIGHT, 4],
        ),
        helper.make_node("Conv", ["cols", "weight", "bias"], ["local"], kernel_shape=[1, 1]),
        helper.make_node("ReduceMean", ["cols"], ["width_mean"], axes=[3], keepdims=1),
        helper.make_node("Conv", ["width_mean", "global_weight"], ["global"], kernel_shape=[1, 1]),
        helper.make_node("Add", ["local", "global"], ["logits"]),
        helper.make_node("Squeeze", ["logits", "squeeze_axes"], ["logits_nct"]),
        helper.make_node("Transpose", ["logits_nct"], ["logits_tnc"], perm=[2, 0, 1]),
        helper.make_node("LogSoftmax", ["logits_tnc"], ["chars"], axis=2),
    ]
    graph = helper.make_graph(
        nodes,
        "locr_synthetic_recognition",
        [x],
        [y],
        initializer=[
            const("weight", weight),
            const("bias", bias),
            const("global_weight", global_weight),
            numpy_helper.from_array(np.array([2], dtype=np.int64), "squeeze_axes"),
        ],
    )
    return helper.make_model(graph, opset_imports=OPSET)


def rten_convert():
    # Prefer the script next to the running interpreter (virtualenvs).
    local = os.path.join(os.path.dirname(sys.executable), "rten-convert")
    return local if os.path.exists(local) else shutil.which("rten-convert") or "rten-convert"


def main():
    for name, model in [
        ("text-detection", detection_model()),
        ("text-recognition", recognition_model()),
    ]:
        onnx.checker.check_model(model)
        onnx_path = os.path.join(HERE, f"{name}.onnx")
        rten_path = os.path.join(HERE, f"{name}.rten")
        onnx.save(model, onnx_path)
        subprocess.run([rten_convert(), onnx_path, rten_path], check=True)
        os.remove(onnx_path)
        print(f"wrote {rten_path} ({os.path.getsize(rten_path)} bytes)")


if __name__ == "__main__":
    main()
