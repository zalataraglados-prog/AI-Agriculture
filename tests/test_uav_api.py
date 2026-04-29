import requests
import pytest
import time

BASE_URL = "http://127.0.0.1:8088"

def test_uav_full_flow():
    # 1. Create Mission
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions", json={"plantation_id": 1, "mission_name": "test"})
    assert res.status_code == 200
    
    # 2. Register Orthomosaic
    res = requests.post(f"{BASE_URL}/api/v1/uav/missions/1/orthomosaic")
    assert res.status_code == 200
    
    # 3. Create mock detections
    res = requests.post(f"{BASE_URL}/api/v1/uav/orthomosaics/1/detections/mock")
    assert res.status_code == 200
    assert res.json().get("status") == "ok"
    
    # 4. Confirm detection
    res = requests.post(f"{BASE_URL}/api/v1/uav/detections/1/confirm")
    assert res.status_code == 200
    tree_code = res.json().get("tree_code")
    assert tree_code == "OP-000001"
    
    # 5. Get tree
    res = requests.get(f"{BASE_URL}/api/v1/trees/{tree_code}")
    assert res.status_code == 200
    assert res.json()["tree"]["tree_code"] == tree_code
