import unittest
import importlib.util
import json
from pathlib import Path

from gpui_toolkit import scene3d as s3


class Scene3DTests(unittest.TestCase):
    def test_surface_spec_matches_rust_shape(self):
        spec = s3.surface(
            "dispersion",
            z=[[1.0, 2.0], [3.0, 4.0]],
            x=[20.0, 20000.0],
            y=[-90.0, 90.0],
            colormap="turbo",
            x_log=True,
            wireframe=True,
            camera=s3.orbit(distance=3.5, azimuth=60.0, elevation=25.0),
            interactions=["orbit", "pan", "zoom", "reset"],
        ).to_spec()

        self.assertEqual(spec["kind"], "surface")
        self.assertEqual(spec["z"], {"values": [1.0, 2.0, 3.0, 4.0], "width": 2, "height": 2})
        self.assertEqual(spec["camera"]["kind"], "orbit")
        self.assertEqual(spec["interactions"], ["orbit", "pan", "zoom", "reset"])

    def test_scene_accepts_future_mesh_nodes(self):
        scene = s3.scene(
            "model",
            children=[
                s3.mesh(
                    "speaker",
                    vertices=[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)],
                    indices=[0, 1, 2],
                    material=s3.material("#88ccff", opacity=0.8),
                )
            ],
        ).to_spec()

        self.assertEqual(scene["children"][0]["kind"], "mesh")
        self.assertAlmostEqual(scene["children"][0]["material"]["color"]["b"], 1.0)

    def test_examples_build_json_specs(self):
        examples_dir = Path(__file__).parents[1] / "examples"
        expected = {
            "surface_dispersion.py": "surface",
            "lines_orbit.py": "lines",
            "mesh_scene.py": None,
        }

        for filename, kind in expected.items():
            with self.subTest(filename=filename):
                path = examples_dir / filename
                module_spec = importlib.util.spec_from_file_location(path.stem, path)
                self.assertIsNotNone(module_spec)
                self.assertIsNotNone(module_spec.loader)
                module = importlib.util.module_from_spec(module_spec)
                module_spec.loader.exec_module(module)

                spec = module.build_spec()
                json.dumps(spec)
                if kind is None:
                    self.assertIn("children", spec)
                else:
                    self.assertEqual(spec["kind"], kind)


if __name__ == "__main__":
    unittest.main()
