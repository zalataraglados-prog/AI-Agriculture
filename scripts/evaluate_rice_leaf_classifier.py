import sys
from pathlib import Path

# Add project root to sys.path if needed
PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from ai_engine.crops.rice.training.evaluate import main

if __name__ == "__main__":
    print("\n" + "!"*60)
    print("DEPRECATED: scripts/evaluate_rice_leaf_classifier.py is moved.")
    print("New location: ai_engine/crops/rice/training/evaluate.py")
    print("Executing the new module now...")
    print("!"*60 + "\n")
    main()
