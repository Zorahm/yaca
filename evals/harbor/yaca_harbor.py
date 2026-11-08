import hashlib
import json
import os
import shlex
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Literal

from harbor.agents.base import BaseAgent
from harbor.agents.options import AgentOptions
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext


@dataclass(frozen=True)
class InferenceRoute:
    provider_id: str
    base_url: str
    token_env: str
    model_id: str
    display_name: str
    input_price: float
    output_price: float


ROUTES = {
    "huggingface/deepseek-ai/DeepSeek-V4.1-Flash:deepinfra": InferenceRoute(
        provider_id="hf-harbor",
        base_url="https://router.huggingface.co/v1",
        token_env="HF_TOKEN",
        model_id="deepseek-ai/DeepSeek-V4.1-Flash:deepinfra",
        display_name="DeepSeek V4.1 Flash via HF and DeepInfra",
        input_price=0.2,
        output_price=0.6,
    ),
    "deepseek/deepseek-flash": InferenceRoute(
        provider_id="deepseek",
        base_url="https://api.deepseek.com",
        token_env="DEEPSEEK_API_KEY",
        model_id="deepseek-flash",
        display_name="DeepSeek V4.1 Flash 1017 via DeepSeek",
        # Use peak rates for the estimate. Actual DeepSeek billing varies by time.
        input_price=0.3,
        output_price=1.2,
    ),
}


def linux_binary() -> Path:
    value = os.environ.get("YACA_HARBOR_BINARY")
    if not value:
        raise ValueError("Set YACA_HARBOR_BINARY to the Linux yaca executable.")
    path = Path(value).expanduser().resolve()
    with path.open("rb") as executable:
        if executable.read(4) != b"\x7fELF":
            raise ValueError(f"Expected a Linux ELF executable: {path}")
    return path


def linux_runtime() -> Path | None:
    value = os.environ.get("YACA_HARBOR_RUNTIME")
    if not value:
        return None
    path = Path(value).expanduser().resolve()
    if not (path / "ld-linux").is_file():
        raise ValueError(f"Expected the Linux dynamic loader at {path / 'ld-linux'}")
    return path


class YacaOptions(AgentOptions):
    effort: Literal["low", "high"] = "low"


class YacaAgent(BaseAgent):
    options_model = YacaOptions
    _version: str | None = None

    @classmethod
    def preflight(cls, kwargs=None, env=None):
        super().preflight(kwargs, env)
        linux_binary()
        linux_runtime()

    @staticmethod
    def name() -> str:
        return "yaca"

    def version(self) -> str | None:
        return self._version

    async def setup(self, environment: BaseEnvironment) -> None:
        route = ROUTES.get(self.model_name)
        if route is None:
            raise ValueError(f"Unsupported Harbor model: {self.model_name!r}")
        self._route = route

        binary = linux_binary()
        with binary.open("rb") as executable:
            self._version = hashlib.file_digest(executable, "sha256").hexdigest()[:12]

        result = await environment.exec("mktemp -d /tmp/yaca-harbor.XXXXXXXX")
        if result.return_code:
            raise RuntimeError(f"Could not create the Yaca setup directory: {result.stderr}")
        run_dir = PurePosixPath(result.stdout.strip())
        container_binary = str(run_dir / "yaca")
        self._config = str(run_dir / "config")
        self._command = [container_binary]

        runtime = linux_runtime()
        if runtime is not None:
            container_runtime = str(run_dir / "runtime")
            loader = str(run_dir / "runtime" / "ld-linux")
            result = await environment.exec(shlex.join(["mkdir", container_runtime]))
            if result.return_code:
                raise RuntimeError(f"Could not create the Yaca runtime directory: {result.stderr}")
            await environment.upload_dir(runtime, container_runtime)
            self._command = [loader, "--library-path", container_runtime, container_binary]

        providers = {"list": [{
            "id": route.provider_id,
            "displayName": route.display_name,
            "protocol": "openai",
            "baseUrl": route.base_url,
            "models": [{
                "id": route.model_id,
                "displayName": route.display_name,
                "contextMax": 1048576,
                "maxOutput": 393216,
                "pricing": {"input": route.input_price, "output": route.output_price},
                "cacheRead": 0.006,
                "supportsEffort": True,
                "reasoning": {"supportedEfforts": ["low", "high"]},
            }],
        }]}
        await environment.upload_file(binary, container_binary)
        executables = [container_binary] if runtime is None else [container_binary, loader]
        result = await environment.exec(shlex.join(["chmod", "755", *executables]), user="root")
        if result.return_code:
            raise RuntimeError(f"Could not make the Yaca binary executable: {result.stderr}")

        config_dir = shlex.quote(self._config + "/Yaca")
        config_file = shlex.quote(self._config + "/Yaca/providers.json")
        config_json = shlex.quote(json.dumps(providers))
        result = await environment.exec(
            f"mkdir -p {config_dir} && printf '%s' {config_json} >{config_file}"
        )
        if result.return_code:
            raise RuntimeError(f"Could not write providers.json: {result.stderr}")

        result = await environment.exec(shlex.join([*self._command, "--help"]))
        if result.return_code:
            details = result.stderr or result.stdout or f"exit status {result.return_code}"
            raise RuntimeError(f"Linux Yaca executable failed its startup check: {details}")

    async def run(self, instruction: str, environment: BaseEnvironment, context: AgentContext) -> None:
        route = self._route
        token = os.environ.get(route.token_env, "").strip()
        if not token:
            raise ValueError(f"Set {route.token_env} for the selected inference route.")

        stdout = shlex.quote(str(self.environment_logs_dir / "yaca.jsonl"))
        stderr = shlex.quote(str(self.environment_logs_dir / "yaca.stderr"))
        argv = [
            *self._command, "--project", "/app", "--provider", route.provider_id,
            "--model", route.model_id, "--mode", "auto", "--permissions", "allow",
            "--effort", self.options.effort, "--jsonl", "--", instruction,
        ]
        key_env = route.provider_id.upper().replace("-", "_") + "_API_KEY"
        result = await environment.exec(
            f"exec {shlex.join(argv)} >{stdout} 2>{stderr}",
            cwd="/app",
            env={
                "XDG_CONFIG_HOME": self._config,
                key_env: token,
            },
        )
        if result.return_code:
            raise RuntimeError(f"Yaca exited with status {result.return_code}. See yaca.jsonl and yaca.stderr.")

    def populate_context_post_run(self, context: AgentContext) -> None:
        log = self.logs_dir / "yaca.jsonl"
        if not log.is_file():
            return
        usage = None
        with log.open(encoding="utf-8") as events:
            for line in events:
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    # A timeout can leave a partial event on the last line.
                    continue
                if event["type"] == "usageUpdated":
                    usage = event["usage"]
        if usage is None:
            return

        counts = usage["total"]
        context.n_input_tokens = counts["input"] + counts["cacheRead"] + counts["cacheWrite"]
        context.n_cache_tokens = counts["cacheRead"]
        context.n_output_tokens = counts["output"]
        context.metadata = {**(context.metadata or {}), "yaca_cost_estimate_usd": usage["costUsd"]}
