from __future__ import annotations

import subprocess
from pathlib import Path

import yaml

from tonggraph.server.config import parse_config


ROOT = Path(__file__).resolve().parents[2]


def test_deploy_config_template_parses_with_env_tokens(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
    monkeypatch.setenv("TONGGRAPH_ADMIN_TOKEN", "admin-test-token")
    monkeypatch.setenv("TONGGRAPH_ALICE_TOKEN", "alice-test-token")
    monkeypatch.setenv("TONGGRAPH_READER_TOKEN", "reader-test-token")
    raw = yaml.safe_load((ROOT / "deploy" / "tonggraph-server.yml").read_text(encoding="utf-8"))

    config = parse_config(raw, base_dir=tmp_path)

    assert config.host == "127.0.0.1"
    assert config.port == 8719
    assert config.auth_mode == "token"
    assert config.users["admin"].admin is True
    assert config.users["admin"].token == "admin-test-token"
    assert config.users["alice"].graphs == {"shared_kg": "write"}
    assert config.users["reader"].graphs == {"shared_kg": "read"}
    assert config.graphs["shared_kg"].is_absolute() is True
    assert str(config.graphs["shared_kg"]).startswith(str(tmp_path))


def test_server_scripts_are_valid_bash() -> None:
    scripts = [
        ROOT / "scripts" / "server" / "start.sh",
        ROOT / "scripts" / "server" / "health.sh",
        ROOT / "scripts" / "server" / "smoke.sh",
    ]
    for script in scripts:
        result = subprocess.run(["bash", "-n", str(script)], cwd=ROOT, text=True, capture_output=True)
        assert result.returncode == 0, result.stderr


def test_deployment_assets_do_not_embed_local_experiment_tokens() -> None:
    checked = [
        ROOT / "deploy" / "tonggraph-server.yml",
        ROOT / "deploy" / "tonggraph-server.env.example",
        ROOT / "deploy" / "systemd" / "tonggraph-server.service",
        ROOT / "scripts" / "server" / "start.sh",
        ROOT / "scripts" / "server" / "health.sh",
        ROOT / "scripts" / "server" / "smoke.sh",
    ]
    forbidden = {
        "admin-dev-token",
        "shuyi-dev-token",
        "keshu-dev-token",
        "xiaobo-dev-token",
        "avertic",
        "shuyi",
        "keshu",
        "xiaobo",
    }
    for path in checked:
        text = path.read_text(encoding="utf-8")
        for token in forbidden:
            assert token not in text, f"{path} embeds local experiment value {token!r}"
