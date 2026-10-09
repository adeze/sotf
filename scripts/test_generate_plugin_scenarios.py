"""Regression coverage for sparse GPUI plugin parameter IDs."""
import importlib.util
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory


class SparseParameterIdsTest(unittest.TestCase):
    def test_generator_uses_sparse_runtime_indices(self):
        path = Path(__file__).with_name("generate_plugin_scenarios.py")
        spec = importlib.util.spec_from_file_location("plugin_scenarios", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        calls = []
        values = {
            "plugins.plugin.2.type": "Dynamic EQ",
            "plugins.plugin.2.param_indices": [0, 1, 100, 110],
        }
        for index in (0, 1, 100, 110):
            values.update({
                f"plugins.plugin.2.param.{index}.name": f"Parameter {index}",
                f"plugins.plugin.2.param.{index}.type": "float",
                f"plugins.plugin.2.param.{index}.min": 0.0,
                f"plugins.plugin.2.param.{index}.max": 1.0,
            })
        module.query = lambda _base, key: values[key]
        module.action = lambda _base, name, payload: calls.append((name, payload))
        scenario = module.generate_for_type("http://unused", "DynamicEq", 2)
        self.assertIn("# 4 queryable parameter ID(s): [0, 1, 100, 110]", scenario)
        for index in (0, 1, 100, 110):
            self.assertIn(f"plugin_param_set 2 {index} 0.3", scenario)
        self.assertNotIn("stopping at parameter", scenario)
        self.assertEqual(calls[0][0], "PluginClear")
        self.assertEqual(calls[-1], ("PluginRemove", {"index": 2}))

    def test_suite_membership_rejects_omission(self):
        path = Path(__file__).with_name("generate_plugin_scenarios.py")
        spec = importlib.util.spec_from_file_location("plugin_scenarios", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with TemporaryDirectory() as directory:
            module.SUITE_PATH = Path(directory) / "plugins.toml"
            module.SUITE_PATH.write_text('[[scenario]]\npath = "crates/sotf-dev-driver/scenarios/plugins/Gain.scn"\n')
            module.verify_suite_membership(["Gain"])
            with self.assertRaisesRegex(RuntimeError, "missing=.*DynamicEq"):
                module.verify_suite_membership(["Gain", "DynamicEq"])


if __name__ == "__main__":
    unittest.main()
