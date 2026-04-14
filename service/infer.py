import argparse
import json
import sys
from pathlib import Path

from PIL import Image

try:
    from service.core.rice_leaf_classifier import RiceLeafClassifier
    from service.adapters.image_adapter import load_image_from_path
except ModuleNotFoundError:
    # Support running `python service/infer.py` from the project root.
    project_root = Path(__file__).resolve().parent.parent
    if str(project_root) not in sys.path:
        sys.path.insert(0, str(project_root))
    from service.core.rice_leaf_classifier import RiceLeafClassifier
    from service.adapters.image_adapter import load_image_from_path


def predict_image_file(
    image_path: str,
    checkpoint_path: str,
    labels_file: str = "models/rice_leaf_classifier/labels.json",
    config_file: str = "models/rice_leaf_classifier/config.json",
    top_k: int = 3
):
    image = load_image_from_path(image_path)
    classifier = RiceLeafClassifier(
        checkpoint_path=checkpoint_path,
        labels_file=labels_file,
        config_file=config_file
    )
    return classifier.predict(image, top_k=top_k)


def main():
    parser = argparse.ArgumentParser(description="Run local inference for rice leaf classifier")
    parser.add_argument("--image-path", required=True, help="Path to input image")
    parser.add_argument(
        "--checkpoint-path",
        required=True,
        help="Path to best_model.pth or final_model.pth"
    )
    parser.add_argument(
        "--labels-file",
        default="models/rice_leaf_classifier/labels.json"
    )
    parser.add_argument(
        "--config-file",
        default="models/rice_leaf_classifier/config.json"
    )
    parser.add_argument(
        "--top-k",
        type=int,
        default=3
    )

    args = parser.parse_args()

    result = predict_image_file(
        image_path=args.image_path,
        checkpoint_path=args.checkpoint_path,
        labels_file=args.labels_file,
        config_file=args.config_file,
        top_k=args.top_k
    )

    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
