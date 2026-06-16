"""Generic callback injection — auto-discovers and wraps callable attributes.

This script is a fallback for frameworks without a specific adapter.
It uses reflection to find methods that look like callbacks/hooks
and wraps them to emit events.
"""

import json
import sys
import re

# Patterns that match callback-like method names
CALLBACK_PATTERNS = [
    # (compiled_regex, event_type)
    (re.compile(r"^on_(.+)$"), "hook:{name}"),           # on_tool_start → hook:tool_start
    (re.compile(r"^(.+)_callback$"), "callback:{name}"), # on_tool → callback:tool
    (re.compile(r"^(.+)_hook$"), "hook:{name}"),         # pre_hook → hook:pre
    (re.compile(r"^handle_(.+)$"), "handle:{name}"),     # handle_tool → handle:tool
    (re.compile(r"^_on_(.+)$"), "hook:{name}"),          # _on_start → hook:start
]


def discover_callbacks(obj, max_depth=2):
    """Discover callback-like methods on an object.

    Returns list of (attr_name, event_type) tuples.
    """
    callbacks = []
    visited = set()

    def _scan(o, depth):
        if depth > max_depth or id(o) in visited:
            return
        visited.add(id(o))

        for attr_name in dir(o):
            if attr_name.startswith("__"):
                continue

            try:
                attr = getattr(o, attr_name, None)
            except Exception:
                continue

            if attr is None or not callable(attr):
                continue

            # Check against patterns
            for pattern, event_template in CALLBACK_PATTERNS:
                m = pattern.match(attr_name)
                if m:
                    event_type = event_template.replace("{name}", m.group(1) if m.lastindex else attr_name)
                    callbacks.append((attr_name, event_type))
                    break

    _scan(obj, 0)
    return callbacks


def make_wrapper(event_type: str, attr_name: str, session_id: str, framework: str):
    """Create a wrapper function that emits events."""

    def wrapper(*args, **kwargs):
        data = {
            "event": event_type,
            "framework": framework,
            "session_id": session_id,
            "method": attr_name,
            "args_count": len(args),
        }

        if args:
            try:
                data["first_arg"] = repr(args[0])[:500]
            except Exception:
                pass

        print(json.dumps(data, ensure_ascii=False))
        sys.stdout.flush()

        if hasattr(wrapper, "_original") and wrapper._original is not None:
            return wrapper._original(*args, **kwargs)

    return wrapper


def patch_agent(obj, session_id: str, framework: str):
    """Auto-discover and patch callback methods on an agent object.

    Args:
        obj: The agent instance.
        session_id: Current session ID.
        framework: Framework identifier.
    """
    callbacks = discover_callbacks(obj)

    for attr_name, event_type in callbacks:
        try:
            original = getattr(obj, attr_name)
            wrapper = make_wrapper(event_type, attr_name, session_id, framework)
            wrapper._original = original
            setattr(obj, attr_name, wrapper)
        except Exception as e:
            print(json.dumps({
                "event": "patch:error",
                "method": attr_name,
                "error": str(e),
            }), file=sys.stderr)
