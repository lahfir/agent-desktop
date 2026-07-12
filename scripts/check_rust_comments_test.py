import importlib.util
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("check_rust_comments.py")
SPEC = importlib.util.spec_from_file_location("check_rust_comments", MODULE_PATH)
COMMENTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMMENTS)


class RustCommentScannerTests(unittest.TestCase):
    def test_ignores_doc_comments_and_comment_markers_in_strings(self):
        source = '''
/// Public documentation.
//! Module documentation.
let url = "https://example.test/path";
let marker = "/* not a comment */";
let raw = r###"// still text /* still text */"###;
'''

        self.assertEqual(COMMENTS.forbidden_comments(source), [])

    def test_reports_line_and_block_comments_only_in_rust_code(self):
        source = '''
// standalone
let value = 1; // trailing
/* block */
let other = 2; /* trailing block */
'''

        findings = COMMENTS.forbidden_comments(source)

        self.assertEqual(
            findings,
            [
                (2, "non-doc line comment"),
                (3, "end-of-line comment"),
                (4, "block comment"),
                (5, "block comment"),
            ],
        )

    def test_nested_doc_blocks_do_not_expose_inner_markers(self):
        source = "/** docs with /* nested */ text */\nlet value = 1;\n"

        self.assertEqual(COMMENTS.forbidden_comments(source), [])


if __name__ == "__main__":
    unittest.main()
