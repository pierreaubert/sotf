"""Regression tests for Rust field counting in the structural QA gate."""

import unittest

from check_struct_sizes import count_fields


class FieldCountTests(unittest.TestCase):
    def test_comments_and_generic_arguments_are_not_fields(self):
        source = '''{
            /// A mapping from names, IDs, and channels.
            mapping: HashMap<String, Vec<(usize, usize)>>,
            #[doc = "Commas, braces }, and parentheses ) are documentation."]
            callback: fn() -> Result<(), Error>,
            // The trailing comma is optional.
            enabled: bool
        }'''
        self.assertEqual(count_fields(source, 0), 3)

    def test_single_and_tuple_fields_without_trailing_comma(self):
        self.assertEqual(count_fields("{ value: bool }", 0), 1)
        self.assertEqual(count_fields("(HashMap<A, B>, bool)", 0), 2)
        self.assertEqual(count_fields("{}", 0), 0)

    def test_real_over_limit_struct_remains_detected(self):
        source = "{" + ",".join(f"field_{i}: bool" for i in range(31)) + "}"
        self.assertEqual(count_fields(source, 0), 31)


if __name__ == "__main__":
    unittest.main()
