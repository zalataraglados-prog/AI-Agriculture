import sys
import subprocess
from pathlib import Path

def main():
    print("\n" + "!"*60)
    print("DEPRECATION WARNING: scripts/train_rice_leaf_classifier.py is moved.")
    print("New location: ai_engine/crops/rice/training/train_rice_leaf_classifier.py")
    print("!"*60 + "\n")
    
    new_script = Path(__file__).parent.parent / "ai_engine" / "crops" / "rice" / "training" / "train_rice_leaf_classifier.py"
    if not new_script.exists():
        print(f"Error: New script location not found at {new_script}")
        sys.exit(1)
        
    subprocess.run([sys.executable, str(new_script)] + sys.argv[1:])

if __name__ == "__main__":
    main()
