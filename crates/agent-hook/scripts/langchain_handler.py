"""LangChain callback handler — bridges LangChain events to the agent-hook protocol.

This handler is injected into a LangChain agent's callback list at runtime.
It prints structured JSON to stdout which the Rust ProcessWrapper captures
and normalizes into AgentEvents.
"""

import json
from datetime import datetime, timezone


class HookHandler:
    """LangChain BaseCallbackHandler that emits structured events."""

    def __init__(self, session_id: str, framework: str):
        self._session_id = session_id
        self._framework = framework

    def _emit(self, event_type: str, **data):
        """发送符合统一协议的事件"""
        msg = {
            "event": event_type,
            "framework": self._framework,
            "session_id": self._session_id,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }
        # 合并数据字段
        msg.update(data)
        print(json.dumps(msg, ensure_ascii=False))

    # ── LLM events ──

    def on_llm_start(self, serialized, prompts, *, run_id, **kwargs):
        self._emit(
            "agent:step",
            model=serialized.get("name", "unknown"),
            tool_call_id=str(run_id),
        )

    def on_llm_new_token(self, token, *, run_id, **kwargs):
        self._emit("message:delta", text=token)

    def on_llm_end(self, response, *, run_id, **kwargs):
        self._emit("message:stream_end")

    def on_llm_error(self, error, *, run_id, **kwargs):
        self._emit("agent:error", error=str(error))

    # ── Tool events ──

    def on_tool_start(self, serialized, input_str, *, run_id, **kwargs):
        self._emit(
            "tool:start",
            tool_name=serialized.get("name", "unknown"),
            tool_input=input_str[:500] if isinstance(input_str, str) else input_str,
            tool_call_id=str(run_id),
        )

    def on_tool_end(self, output, *, run_id, **kwargs):
        self._emit(
            "tool:complete",
            tool_response=str(output)[:500],
            success=True,
            tool_call_id=str(run_id),
        )

    def on_tool_error(self, error, *, run_id, **kwargs):
        self._emit(
            "tool:error",
            error=str(error),
            tool_call_id=str(run_id),
        )

    # ── Chain events ──

    def on_chain_start(self, serialized, inputs, *, run_id, **kwargs):
        self._emit(
            "chain:start",
            name=serialized.get("name", "unknown"),
            tool_call_id=str(run_id),
        )

    def on_chain_end(self, outputs, *, run_id, **kwargs):
        self._emit("chain:end", tool_call_id=str(run_id))

    def on_chain_error(self, error, *, run_id, **kwargs):
        self._emit("chain:error", error=str(error), tool_call_id=str(run_id))

    # ── Agent events ──

    def on_agent_action(self, action, *, run_id, **kwargs):
        self._emit(
            "tool:start",
            tool_name=action.tool,
            tool_input=str(action.tool_input)[:500],
            tool_call_id=str(run_id),
        )

    def on_agent_finish(self, output, *, run_id, **kwargs):
        self._emit("agent:end", tool_call_id=str(run_id))
