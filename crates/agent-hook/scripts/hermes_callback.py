"""Hermes Agent callback injection — wraps Hermes AIAgent callbacks.

This script is executed in the Python process that hosts the Hermes agent.
It discovers the agent instance and replaces its callback attributes with
wrappers that emit structured JSON events to stdout.

Usage (from Rust):
    python_patcher.patch_agent(py, agent_dict, Some("hermes"))
"""

import json
import sys
from datetime import datetime, timezone


def make_callback_wrapper(event_type: str, callback_name: str, session_id: str, framework: str):
    """Create a callback function that emits a structured event."""

    def wrapper(*args, **kwargs):
        data = {
            "event": event_type,
            "framework": framework,
            "session_id": session_id,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }

        # 根据事件类型提取特定字段
        if event_type == "message:delta" and args:
            # 消息增量 - 提取文本
            try:
                data["text"] = str(args[0])[:500]
            except Exception:
                pass
        elif event_type == "tool:start" and args:
            # 工具开始 - 提取工具名和输入
            try:
                data["tool_name"] = str(args[0])
                if len(args) > 1:
                    data["tool_input"] = args[1] if isinstance(args[1], dict) else str(args[1])[:500]
            except Exception:
                pass
        elif event_type == "agent:step":
            # Agent 步骤
            if args:
                try:
                    data["iteration"] = int(args[0]) if args[0] is not None else 0
                except (ValueError, TypeError):
                    pass
        elif event_type == "message:interim" and args:
            # 临时消息 - 提取文本
            try:
                data["text"] = str(args[0])[:500]
            except Exception:
                pass
        elif event_type == "thinking:delta" and args:
            # 思考增量
            try:
                data["text"] = str(args[0])[:500]
            except Exception:
                pass
        elif event_type == "system:status":
            # 系统状态
            if args:
                try:
                    data["kind"] = str(args[0])
                except Exception:
                    pass
            if len(args) > 1:
                try:
                    data["message"] = str(args[1])[:500]
                except Exception:
                    pass

        print(json.dumps(data, ensure_ascii=False))
        sys.stdout.flush()

        # Call original if provided via closure
        if hasattr(wrapper, "_original") and wrapper._original is not None:
            return wrapper._original(*args, **kwargs)

    return wrapper


# Callback name → event type mapping
CALLBACK_EVENT_MAP = {
    "stream_delta_callback": "message:delta",
    "tool_start_callback": "tool:start",
    "step_callback": "agent:step",
    "status_callback": "system:status",
    "interim_assistant_callback": "message:interim",
    "approval_callback": "approval:request",
    "thinking_callback": "thinking:delta",
    "reasoning_callback": "reasoning:available",
}


def patch_agent_dict(agent_dict: dict, session_id: str, framework: str):
    """Patch an agent instance's __dict__ by replacing callback attributes.

    Args:
        agent_dict: The agent instance's __dict__ (mutable).
        session_id: Current session ID for event correlation.
        framework: Framework identifier (e.g. "hermes").
    """
    for callback_name, event_type in CALLBACK_EVENT_MAP.items():
        if callback_name in agent_dict:
            original = agent_dict[callback_name]
            wrapper = make_callback_wrapper(event_type, callback_name, session_id, framework)
            wrapper._original = original
            agent_dict[callback_name] = wrapper
