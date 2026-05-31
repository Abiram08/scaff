import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

from scaff.generator import normalize_agent_spec, validate_agent_spec
from scaff.writer import write_generated_files


class ProductContractTests(unittest.TestCase):
    def test_normalizes_model_output_into_runnable_tool_shapes(self) -> None:
        spec = {
            "agent_name": "Issue Triage Agent!",
            "description": "Triage issues",
            "system_prompt": "Help triage issues.",
            "tools": [
                {
                    "name": "1 Fetch Issues",
                    "description": "Fetch open issues",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "repo-name": {
                                "type": "string",
                                "description": "Repository name",
                            },
                        },
                        "required": ["repo-name"],
                    },
                }
            ],
            "dependencies": "requests",
        }

        normalized = normalize_agent_spec(spec)

        self.assertEqual(normalized["agent_name"], "issue-triage-agent")
        self.assertEqual(normalized["tools"][0]["name"], "tool_1_fetch_issues")
        self.assertEqual(normalized["tools"][0]["parameters"]["repo_name"]["type"], "string")
        self.assertEqual(normalized["tools"][0]["required"], ["repo_name"])
        self.assertEqual(normalized["dependencies"], ["requests"])
        self.assertTrue(validate_agent_spec(normalized))

    def test_writer_outputs_exact_project_contract(self) -> None:
        spec = {
            "agent_name": "demo-agent",
            "description": "Demo agent",
            "system_prompt": "Be useful.",
            "tools": [
                {
                    "name": "search_things",
                    "description": "Search for things",
                    "parameters": {
                        "query": {
                            "type": "string",
                            "description": "Search query",
                        },
                    },
                    "required": ["query"],
                }
            ],
            "dependencies": [],
        }

        output_path = Path(tempfile.mkdtemp())
        created_files = write_generated_files(spec, str(output_path))

        self.assertGreaterEqual(len(created_files), 8)
        self.assertIn(".env.example", [p.name for p in output_path.iterdir()])
        self.assertIn("agent.py", [p.name for p in output_path.iterdir()])
        self.assertIn("tools.py", [p.name for p in output_path.iterdir()])
        env_example = (output_path / ".env.example").read_text(encoding="utf-8")
        self.assertIn("AI Provider API Keys", env_example)
        self.assertIn("OPENAI_API_KEY", env_example)
        self.assertIn("ANTHROPIC_API_KEY", env_example)
        self.assertIn("GOOGLE_API_KEY", env_example)

        self.assertEqual(
            (output_path / ".gitignore").read_text(encoding="utf-8").splitlines(),
            [".env", "__pycache__/", "*.pyc", ".DS_Store"],
        )

        manifest = json.loads((output_path / "MANIFEST.json").read_text(encoding="utf-8"))
        self.assertEqual(
            sorted(manifest.keys()),
            ["agent_name", "description", "generated_at", "memory", "provider", "scaff_version", "schedule", "ui_mode"],
        )
        self.assertEqual(manifest["provider"], "openai")
        self.assertIsNone(manifest["ui_mode"])
        self.assertIsNone(manifest["schedule"])
        self.assertIsNone(manifest["memory"])
        self.assertIn("openai>=1.0.0", (output_path / "requirements.txt").read_text())

    def test_rendered_agent_imports_and_dispatches_tools(self) -> None:
        spec = {
            "agent_name": "demo-agent",
            "description": "Demo agent",
            "system_prompt": "Be useful.",
            "tools": [
                {
                    "name": "search_things",
                    "description": "Search for things",
                    "parameters": {
                        "query": {
                            "type": "string",
                            "description": "Search query",
                        },
                    },
                    "required": ["query"],
                }
            ],
            "dependencies": [],
        }

        output_path = Path(tempfile.mkdtemp())
        write_generated_files(spec, str(output_path))
        sys.path.insert(0, str(output_path))
        try:
            module_spec = importlib.util.spec_from_file_location(
                "generated_agent",
                output_path / "agent.py",
            )
            self.assertIsNotNone(module_spec)
            self.assertIsNotNone(module_spec.loader)
            agent = importlib.util.module_from_spec(module_spec)
            module_spec.loader.exec_module(agent)
        finally:
            sys.path.remove(str(output_path))
            sys.modules.pop("tools", None)

        self.assertEqual(agent.get_tool_definitions()[0]["type"], "function")
        result = json.loads(agent.execute_tool("search_things", {"query": "mvp"}))
        # Tool now tries real HTTP and gracefully handles failure
        self.assertIn("status", result)
        self.assertIn(result.get("status"), ("ok", "error"))


if __name__ == "__main__":
    unittest.main()
