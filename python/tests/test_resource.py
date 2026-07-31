import unittest

from ikashita import (
    ResourcePage,
    ResourceError,
    apply_merge_patch,
    parse_list_query,
    require_object_patch,
)


class ResourceHelpersTest(unittest.TestCase):
    def test_merge_patch_recurses_and_removes_null(self):
        value = {"name": "Ada", "profile": {"active": True, "team": "math"}, "tags": ["old"]}
        patch = {"profile": {"team": "science", "active": None}, "tags": ["new"]}
        self.assertEqual(
            apply_merge_patch(value, patch),
            {"name": "Ada", "profile": {"team": "science"}, "tags": ["new"]},
        )
        self.assertEqual(value["profile"]["team"], "math")

    def test_update_patch_must_be_object(self):
        with self.assertRaises(ResourceError) as caught:
            require_object_patch(["not", "an", "object"])
        self.assertEqual(caught.exception.code, "validation_failed")

    def test_query_parsing_matches_contract(self):
        query = parse_list_query("q=Ada%20Lovelace&sort=-name,email:asc&offset=2&limit=900&ignored=x")
        self.assertEqual(query.q, "Ada Lovelace")
        self.assertEqual([(item.field, item.direction) for item in query.sort], [("name", "desc"), ("email", "asc")])
        self.assertEqual((query.offset, query.limit), (2, 500))

    def test_query_rejects_invalid_values(self):
        with self.assertRaises(ResourceError) as caught:
            parse_list_query("offset=-1")
        self.assertEqual(caught.exception.fields["offset"], "must be a non-negative integer")

    def test_page_normalizes_contract_limit(self):
        self.assertEqual(ResourcePage((), 0, 0, 900).limit, 500)
