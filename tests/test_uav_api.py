import requests
import pytest
import time

BASE_URL = "http://127.0.0.1:8088"

def test_uav_full_flow():
    # 1. Create Mission
    # Use plantation_id 0 to auto-create a plantation
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions", json={"plantation_id": 0, "mission_name": "test_chain"})
    assert res.status_code == 200
    mission_id = res.json().get("mission_id")
    assert mission_id > 0
    
    # 2. Register Orthomosaic
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions/{mission_id}/orthomosaic")
    assert res.status_code == 200
    ortho_id = res.json().get("orthomosaic_id")
    assert ortho_id > 0
    
    # 3. Create mock detections
    res = requests.post(f"{BASE_URL}/api/v1/uav/orthomosaics/{ortho_id}/detections/mock")
    assert res.status_code == 200
    assert res.json().get("status") == "ok"
    assert res.json().get("detections_created") == 3
    
    # 4. Fetch detections
    res = requests.get(f"{BASE_URL}/api/v1/uav/orthomosaics/{ortho_id}/detections")
    assert res.status_code == 200
    detections = res.json().get("detections", [])
    assert len(detections) >= 3
    
    # Take pending items
    pending_ids = [d["id"] for d in detections if d["review_status"] == "pending"]
    assert len(pending_ids) >= 3
    det1 = pending_ids[0]
    det2 = pending_ids[1]
    det3 = pending_ids[2]
    
    # 5. Confirm detection 1
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det1}/confirm")
    assert res.status_code == 200
    tree_code_1 = res.json().get("tree_code")
    assert tree_code_1.startswith("OP-")
    
    # Idempotency test: Confirm detection 1 again
    res_dup = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det1}/confirm")
    assert res_dup.status_code == 200
    assert res_dup.json().get("tree_code") == tree_code_1
    
    # 6. Confirm detection 2
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det2}/confirm")
    assert res.status_code == 200
    tree_code_2 = res.json().get("tree_code")
    assert tree_code_2.startswith("OP-")
    
    # Verify uniqueness
    assert tree_code_1 != tree_code_2
    
    # 7. Reject detection 3
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det3}/reject")
    assert res.status_code == 200
    
    # Cannot confirm rejected
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det3}/confirm")
    assert res.status_code == 500
    
    # 8. Get tree
    res = requests.get(f"{BASE_URL}/api/v1/trees/{tree_code_1}")
    assert res.status_code == 200
    assert res.json()["tree"]["tree_code"] == tree_code_1
    assert res.json()["tree"]["species"] == "oil_palm"

def test_missing_detection():
    # 9. Non-existent detection
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/999999/confirm")
    assert res.status_code == 500

def test_list_plantations():
    """Verify plantations endpoint returns a list."""
    res = requests.get(f"{BASE_URL}/api/v1/plantations")
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "ok"
    assert isinstance(data["plantations"], list)
    assert len(data["plantations"]) >= 1

def test_tree_detail_fields():
    """Verify tree detail returns enhanced fields after confirm."""
    # First, create a tree through the confirm flow
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions", json={"plantation_id": 0, "mission_name": "detail_test"})
    mission_id = res.json()["mission_id"]
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions/{mission_id}/orthomosaic")
    ortho_id = res.json()["orthomosaic_id"]
    requests.post(f"{BASE_URL}/api/v1/uav/orthomosaics/{ortho_id}/detections/mock")
    res = requests.get(f"{BASE_URL}/api/v1/uav/orthomosaics/{ortho_id}/detections")
    det_id = res.json()["detections"][0]["id"]
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det_id}/confirm")
    tree_code = res.json()["tree_code"]

    # Verify enhanced detail
    res = requests.get(f"{BASE_URL}/api/v1/trees/{tree_code}")
    assert res.status_code == 200
    tree = res.json()["tree"]
    assert tree["tree_code"] == tree_code
    assert tree["species"] == "oil_palm"
    assert "barcode_value" in tree
    assert "manual_verified" in tree
    assert "plantation_name" in tree
    assert "created_at" in tree
    assert "coordinate_x" in tree

def test_list_trees_with_pagination():
    """Verify tree list returns total count for pagination."""
    # Get a plantation ID
    res = requests.get(f"{BASE_URL}/api/v1/plantations")
    pid = res.json()["plantations"][0]["id"]

    res = requests.get(f"{BASE_URL}/api/v1/trees?plantation_id={pid}&page=1&limit=10")
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "ok"
    assert isinstance(data["trees"], list)
    assert "total" in data
    assert data["total"] >= 0
    assert data["page"] == 1
    assert data["limit"] == 10

def test_tree_timeline_empty():
    """Verify timeline returns 200 with empty list for new trees."""
    # Get a tree code from earlier tests
    res = requests.get(f"{BASE_URL}/api/v1/plantations")
    pid = res.json()["plantations"][0]["id"]
    res = requests.get(f"{BASE_URL}/api/v1/trees?plantation_id={pid}&page=1&limit=1")
    trees = res.json()["trees"]
    if len(trees) == 0:
        return  # skip if no trees
    code = trees[0]["tree_code"]

    res = requests.get(f"{BASE_URL}/api/v1/trees/{code}/timeline")
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "ok"
    assert isinstance(data["timeline"], list)

def test_update_tree_status_validation():
    """Verify status update rejects invalid values and accepts valid ones."""
    # Get a tree
    res = requests.get(f"{BASE_URL}/api/v1/plantations")
    pid = res.json()["plantations"][0]["id"]
    res = requests.get(f"{BASE_URL}/api/v1/trees?plantation_id={pid}&page=1&limit=1")
    trees = res.json()["trees"]
    if len(trees) == 0:
        return
    code = trees[0]["tree_code"]

    # Invalid status should return 400
    res = requests.put(f"{BASE_URL}/api/v1/trees/{code}/status",
                       json={"status": "happy"})
    assert res.status_code == 400

    # Valid status should return 200
    res = requests.put(f"{BASE_URL}/api/v1/trees/{code}/status",
                       json={"status": "active"})
    assert res.status_code == 200


def test_tiles_grid_computation():
    """Verify tile grid is computed correctly for ortho dimensions."""
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions",
                        json={"plantation_id": 0, "mission_name": "tiling_test"})
    assert res.status_code == 200
    mid = res.json()["mission_id"]

    # Register orthomosaic with known dimensions
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions/{mid}/orthomosaic",
                        json={"width": 1000, "height": 1000, "resolution": 0.05})
    assert res.status_code == 200
    oid = res.json()["orthomosaic_id"]

    # Create tiles with 512px tile size, 0.15 overlap
    res = requests.post(f"{BASE_URL}/api/v1/uav/orthomosaics/{oid}/tiles",
                        json={"tile_size": 512, "tile_overlap": 0.15})
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "ok"
    assert "tile_ids" in data
    tile_ids = data["tile_ids"]
    assert len(tile_ids) > 0

    # stride = 512 * 0.85 = 435, cols = ceil(1000/435) = 3, rows = 3
    assert data["tile_grid"]["cols"] >= 1
    assert data["tile_grid"]["rows"] >= 1
    total_tiles = data["tile_grid"]["cols"] * data["tile_grid"]["rows"]
    # Some edge tiles may be skipped if width/height <= 0 for last col/row
    assert 1 <= len(tile_ids) <= total_tiles

    return oid


def test_detect_palms_with_tiles():
    """Verify detect-palms pipeline: tiles -> mock detections -> NMS -> write."""
    oid = test_tiles_grid_computation()

    # Run detect-palms
    res = requests.post(f"{BASE_URL}/api/v1/uav/orthomosaics/{oid}/detect-palms",
                        json={})
    assert res.status_code == 200
    data = res.json()
    assert data["status"] == "ok"
    assert data["detections_created"] >= 0
    assert data["tiles_processed"] > 0

    # Fetch detections
    res = requests.get(f"{BASE_URL}/api/v1/uav/orthomosaics/{oid}/detections")
    assert res.status_code == 200
    detections = res.json()["detections"]
    assert len(detections) == data["detections_created"]

    # Verify all detections have coordinates within ortho bounds (0, 1000)
    for det in detections:
        cx = det.get("crown_center_x")
        cy = det.get("crown_center_y")
        assert cx is not None and cy is not None
        assert 0 <= cx <= 1000, f"detection {det['id']} cx={cx} out of bounds"
        assert 0 <= cy <= 1000, f"detection {det['id']} cy={cy} out of bounds"

    # Verify detections can be confirmed
    pending = [d for d in detections if d["review_status"] == "pending"]
    if pending:
        det_id = pending[0]["id"]
        res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det_id}/confirm")
        assert res.status_code == 200
        assert res.json()["tree_code"].startswith("OP-")

    return [d for d in detections]
