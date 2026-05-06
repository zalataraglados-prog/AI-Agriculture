"""Oil Palm data importers package.

Each importer converts one specific external data source from raw/ into the
project's unified processed/ format. The principle is:

    raw layer accepts everything; processed/train layer unifies everything.

Adding a new data source only requires writing a new importer file here.
No changes to the core training pipeline or label definitions are needed.

Naming convention: import_{dataset_short_name}.py
Example:
    import_ffb_mendeley_2025.py
    import_ffb_roboflow.py
    import_ganoderma_csv_dataset.py
    import_uav_coco_dataset.py
"""
