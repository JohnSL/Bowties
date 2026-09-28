from __future__ import annotations

import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR))

from cdi_registry import CdiNode  # noqa: E402
from profile_tools import _expand_event_roles  # noqa: E402


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
