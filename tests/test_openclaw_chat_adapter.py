import importlib.util
from unittest import mock
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "cloud" / "scripts" / "openclaw_chat_adapter.py"


def load_adapter_module():
    spec = importlib.util.spec_from_file_location("openclaw_chat_adapter", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_selects_tree_profile_for_tree_lookup():
    adapter = load_adapter_module()
    requests = adapter.select_tool_requests("查询 OP-000048 的树档案", {}, None)
    assert len(requests) == 1
    assert requests[0].name == "query_tree_profile"
    assert requests[0].path == "/tree-profile"
    assert requests[0].params["tree_code"] == "OP-000048"


def test_selects_missing_evidence_for_gap_question():
    adapter = load_adapter_module()
    requests = adapter.select_tool_requests("OP-000048 缺少哪些证据？", {}, None)
    assert len(requests) == 1
    assert requests[0].name == "query_missing_evidence"
    assert requests[0].params["tree_code"] == "OP-000048"


def test_selects_patrol_report_from_context_plantation():
    adapter = load_adapter_module()
    requests = adapter.select_tool_requests("今天优先巡检哪些树？", {"plantation_id": 1}, None)
    assert len(requests) == 1
    assert requests[0].name == "generate_patrol_report"
    assert requests[0].params["plantation_id"] == "1"


def test_selects_plantation_report_from_explicit_id():
    adapter = load_adapter_module()
    requests = adapter.select_tool_requests("plantation_id=12 的 dashboard 报告", {}, None)
    assert len(requests) == 1
    assert requests[0].name == "query_plantation_report"
    assert requests[0].params["plantation_id"] == "12"


def test_compacts_large_tool_context():
    adapter = load_adapter_module()
    text = adapter.compact_tool_context({"source": "test", "blob": "x" * 5000}, 1200)
    assert len(text) <= 1200
    assert '"truncated": true' in text


def test_fetch_tool_propagates_authorization_header():
    adapter = load_adapter_module()
    service = adapter.OpenClawAdapter.__new__(adapter.OpenClawAdapter)
    service.cloud_tool_base_url = "http://127.0.0.1:8088/api/v1/openclaw/tools"
    service.tool_timeout_sec = 1
    service.cloud_tool_auth_bearer = None
    captured = {}

    class FakeResponse:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return False

        def read(self):
            return b'{"status":"ok"}'

    def fake_urlopen(req, timeout):
        captured["authorization"] = req.headers.get("Authorization")
        captured["timeout"] = timeout
        return FakeResponse()

    with mock.patch.object(adapter.urllib.request, "urlopen", side_effect=fake_urlopen):
        payload = service.fetch_tool(
            adapter.ToolRequest("query_tree_profile", "/tree-profile", {"tree_code": "OP-000048"}),
            "Bearer test-token",
        )

    assert payload["status"] == "ok"
    assert captured["authorization"] == "Bearer test-token"
    assert captured["timeout"] == 1


def test_fetch_tool_uses_fallback_bearer_token():
    adapter = load_adapter_module()
    service = adapter.OpenClawAdapter.__new__(adapter.OpenClawAdapter)
    service.cloud_tool_base_url = "http://127.0.0.1:8088/api/v1/openclaw/tools"
    service.tool_timeout_sec = 1
    service.cloud_tool_auth_bearer = "fallback-token"
    captured = {}

    class FakeResponse:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return False

        def read(self):
            return b'{"status":"ok"}'

    def fake_urlopen(req, timeout):
        captured["authorization"] = req.headers.get("Authorization")
        return FakeResponse()

    with mock.patch.object(adapter.urllib.request, "urlopen", side_effect=fake_urlopen):
        service.fetch_tool(adapter.ToolRequest("query_tree_profile", "/tree-profile", {"tree_code": "OP-000048"}))

    assert captured["authorization"] == "Bearer fallback-token"
