from __future__ import annotations

import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR))

from cdi_registry import CdiNode  # noqa: E402
from profile_tools import _expand_event_roles, main  # noqa: E402


class AssembleProfileTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.node_dir = Path(self.temp_dir.name)
        cdi_path = (
            Path(__file__).resolve().parents[4]
            / "docs"
            / "ref"
            / "RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml"
        )
        (self.node_dir / "manual-outline.json").write_text(
            json.dumps(
                {
                    "cdiFile": str(cdi_path),
                    "nodeType": {
                        "manufacturer": "Example Manufacturer",
                        "model": "Example Model",
                    },
                }
            ),
            encoding="utf-8",
        )
        (self.node_dir / "event-roles.json").write_text(
            '{"roles": []}\n',
            encoding="utf-8",
        )
        (self.node_dir / "relevance-rules.json").write_text(
            '{"rules": []}\n',
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def test_assemble_uses_bootstrap_profile_filename_by_default(self) -> None:
        result = main(["assemble", str(self.node_dir)])

        self.assertEqual(result, 0)
        self.assertTrue(
            (
                self.node_dir
                / "Example_Manufacturer_Example_Model.bootstrap.profile.yaml"
            ).is_file()
        )

    def test_assembled_profile_header_warns_against_replacing_graduated_profile(
        self,
    ) -> None:
        main(["assemble", str(self.node_dir)])

        output = (
            self.node_dir
            / "Example_Manufacturer_Example_Model.bootstrap.profile.yaml"
        ).read_text(encoding="utf-8")
        self.assertIn("bootstrap candidate", output)
        self.assertIn(
            "must not replace a graduated bundled profile wholesale",
            output,
        )

    def test_assemble_help_explains_bootstrap_lifecycle_and_filename(self) -> None:
        stdout = io.StringIO()

        with contextlib.redirect_stdout(stdout), self.assertRaises(SystemExit) as exit:
            main(["assemble", "--help"])

        self.assertEqual(exit.exception.code, 0)
        help_text = stdout.getvalue()
        self.assertIn("bootstrap/refinement candidate", help_text)
        self.assertIn("<Manufacturer>_<Model>.bootstrap.profile.yaml", help_text)
        self.assertIn("durable authored runtime source of truth", help_text)
        self.assertIn(
            "must not replace a graduated bundled profile wholesale",
            help_text,
        )

    def test_assemble_ignores_legacy_field_presentation_input(self) -> None:
        (self.node_dir / "field-presentation.json").write_text(
            '{"fieldPresentation": []}\n',
            encoding="utf-8",
        )
        stdout = io.StringIO()

        with contextlib.redirect_stdout(stdout):
            result = main(["assemble", str(self.node_dir)])

        output = (
            self.node_dir
            / "Example_Manufacturer_Example_Model.bootstrap.profile.yaml"
        ).read_text(encoding="utf-8")
        self.assertEqual(result, 0)
        self.assertNotIn("fieldPresentation:", output)
        self.assertNotIn("fieldPresentation:", stdout.getvalue())


class ExpandEventRolesTests(unittest.TestCase):
    def setUp(self) -> None:
        set_aspect = CdiNode(name="set aspect", kind="eventid")
        aspect_set = CdiNode(name="aspect is set", kind="eventid")
        description = CdiNode(name="Description", kind="leaf")
        rule = CdiNode(
            name="Rule",
            kind="group",
            children=[set_aspect, aspect_set, description],
        )
        mast = CdiNode(name="Mast", kind="group", children=[rule])
        segment = CdiNode(name="Rule to Aspect", kind="segment", children=[mast])
        self.root = CdiNode(name="", kind="root", children=[segment])

    def test_expands_sibling_eventids_to_distinct_canonical_targets(self) -> None:
        roles = _expand_event_roles(
            [
                {
                    "cdiPath": "Rule to Aspect/Mast/Rule",
                    "role": "Consumer",
                    "childFields": ["set aspect"],
                },
                {
                    "cdiPath": "Rule to Aspect/Mast/Rule",
                    "role": "Producer",
                    "childFields": ["aspect is set"],
                },
            ],
            self.root,
        )
        self.assertEqual(
            roles,
            [
                {
                    "groupPath": "Rule to Aspect/Mast/Rule/set aspect",
                    "role": "Consumer",
                    "_segment": "Rule to Aspect",
                },
                {
                    "groupPath": "Rule to Aspect/Mast/Rule/aspect is set",
                    "role": "Producer",
                    "_segment": "Rule to Aspect",
                },
            ],
        )

    def test_deduplicates_repeated_role_for_same_leaf(self) -> None:
        entry = {
            "cdiPath": "Rule to Aspect/Mast/Rule",
            "role": "Consumer",
            "childFields": ["set aspect"],
        }

        roles = _expand_event_roles([entry, entry], self.root)

        self.assertEqual(len(roles), 1)

    def test_rejects_unresolvable_child(self) -> None:
        entries = [{
            "cdiPath": "Rule to Aspect/Mast/Rule",
            "role": "Producer",
            "childFields": ["missing"],
        }]

        with self.assertRaisesRegex(ValueError, "Could not resolve event-role child"):
            _expand_event_roles(entries, self.root)

    def test_rejects_non_eventid_child(self) -> None:
        entries = [{
            "cdiPath": "Rule to Aspect/Mast/Rule",
            "role": "Producer",
            "childFields": ["Description"],
        }]

        with self.assertRaisesRegex(ValueError, "is not an EventId"):
            _expand_event_roles(entries, self.root)

    def test_rejects_conflicting_roles_for_same_leaf(self) -> None:
        entries = [
            {
                "cdiPath": "Rule to Aspect/Mast/Rule",
                "role": "Consumer",
                "childFields": ["set aspect"],
            },
            {
                "cdiPath": "Rule to Aspect/Mast/Rule",
                "role": "Producer",
                "childFields": ["set aspect"],
            },
        ]

        with self.assertRaisesRegex(ValueError, "Conflicting event roles"):
            _expand_event_roles(entries, self.root)


if __name__ == "__main__":
    unittest.main()
