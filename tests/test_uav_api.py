import requests
import pytest
import time

BASE_URL = "http://127.0.0.1:8088"

def test_uav_full_flow():
    # 1. Create Mission
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions", json={"plantation_id": 1, "mission_name": "test_chain"})
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
    
    # Take first two pending
    pending_ids = [d["id"] for d in detections if d["review_status"] == "pending"]
    assert len(pending_ids) >= 2
    det1 = pending_ids[0]
    det2 = pending_ids[1]
    
    # 5. Confirm detection 1
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det1}/confirm")
    assert res.status_code == 200
    tree_code_1 = res.json().get("tree_code")
    assert tree_code_1.startswith("OP-")
    
    # 6. Confirm detection 2
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/{det2}/confirm")
    assert res.status_code == 200
    tree_code_2 = res.json().get("tree_code")
    assert tree_code_2.startswith("OP-")
    
    # Verify uniqueness
    assert tree_code_1 != tree_code_2
    
    # 7. Get tree
    res = requests.get(f"{BASE_URL}/api/v1/trees/{tree_code_1}")
    assert res.status_code == 200
    assert res.json()["tree"]["tree_code"] == tree_code_1
    assert res.json()["tree"]["species"] == "oil_palm"
