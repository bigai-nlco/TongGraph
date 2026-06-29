from __future__ import annotations

import os
import subprocess
from pathlib import Path

import yaml

from tonggraph.server.config import parse_config


ROOT = Path(__file__).resolve().parents[2]


def test_deploy_config_template_parses_with_env_tokens(tmp_path: Path, monkeypatch) -> None:  # type: ignore[no-untyped-def]
    monkeypatch.setenv("TONGGRAPH_ADMIN_TOKEN", "admin-test-token")
    monkeypatch.setenv("TONGGRAPH_ALICE_TOKEN", "alice-test-token")
    monkeypatch.setenv("TONGGRAPH_READER_TOKEN", "reader-test-token")
    raw = yaml.safe_load(
        """
        host: 127.0.0.1
        port: 8719
        data_dir: .tonggraph
        graphs:
          shared_kg: shared_kg.db
        auth:
          mode: token
          users:
            admin:
              admin: true
              token_env: TONGGRAPH_ADMIN_TOKEN
              graphs:
                "*": write
            alice:
              token_env: TONGGRAPH_ALICE_TOKEN
              graphs:
                shared_kg: write
            reader:
              token_env: TONGGRAPH_READER_TOKEN
              graphs:
                shared_kg: read
        """
    )

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




def test_start_script_loads_default_env_file(tmp_path: Path) -> None:
    config = tmp_path / "server.yml"
    config.write_text("host: 127.0.0.1\n", encoding="utf-8")
    env_file = tmp_path / "server.env"
    env_file.write_text(
        f"TONGGRAPH_CONFIG={config}\n"
        "TONGGRAPH_ADMIN_TOKEN=loaded-admin-token\n"
        "TONGGRAPH_HOST=127.0.0.2\n"
        "TONGGRAPH_PORT=9999\n",
        encoding="utf-8",
    )
    fake_bin = tmp_path / "tonggraph-server-fake"
    fake_bin.write_text(
        "#!/usr/bin/env bash\n"
        "echo token=${TONGGRAPH_ADMIN_TOKEN}\n"
        'printf "args=%s\n" "$*"\n',
        encoding="utf-8",
    )
    fake_bin.chmod(0o755)

    env = {
        **os.environ,
        "TONGGRAPH_ENV_FILE": str(env_file),
        "TONGGRAPH_USE_UV": "0",
        "TONGGRAPH_SERVER_BIN": str(fake_bin),
    }
    result = subprocess.run(
        [str(ROOT / "scripts" / "server" / "start.sh")],
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0, result.stderr
    assert "token=loaded-admin-token" in result.stdout
    assert f"--config {config}" in result.stdout
    assert "--host 127.0.0.2" in result.stdout
    assert "--port 9999" in result.stdout


def test_deployment_assets_do_not_embed_local_experiment_tokens() -> None:
    checked = [
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
