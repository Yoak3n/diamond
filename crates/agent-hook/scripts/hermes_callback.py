"""Hermes Agent callback injection — wraps Hermes AIAgent callbacks.

This script is executed in the Python process that hosts the Hermes agent.
It discovers the agent instance and replaces its callback attributes with
wrappers that emit structured JSON events to stdout.

Usage (from Rust):
    python_patcher.patch_agent(py, agent_dict, Some("hermes"))
"""

import json
import sys


def make_callback_wrapper(event_type: str, callback_name: str, session_id: str, framework: str):
    """Create a callback function that emits a structured event."""

    def wrapper(*args, **kwargs):
        data = {
            "event": event_type,
            "framework": framework,
            "session_id": session_id,
            "callback": callback_name,
        }

        # Extract first argument if present
        if args:
            try:
                data["arg"] = repr(args[0])[:500]
            except Exception:
                data["arg"] = "<unrepresentable>"

        # Extract kwargs if present
        if kwargs:
            data["kwargs"] = {k: repr(v)[:200] for k, v in list(kwargs.items())[:10]}

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
