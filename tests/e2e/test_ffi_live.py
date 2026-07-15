import ctypes
import unittest

from ffi_live import Action, ref_for


class FfiLiveContractTests(unittest.TestCase):
    def test_action_layout_matches_pinned_c_abi(self):
        self.assertEqual(ctypes.sizeof(Action), 96)

    def test_ref_lookup_reads_typed_native_identifier(self):
        envelope = {
            "data": {
                "tree": {
                    "role": "window",
                    "children": [{
                        "role": "button",
                        "native_id": {"kind": "AX_IDENTIFIER", "value": "delayed-button"},
                        "ref_id": "@snapshot:e3",
                    }],
                }
            }
        }
        self.assertEqual(ref_for(envelope, "delayed-button"), "@snapshot:e3")


if __name__ == "__main__":
    unittest.main()
