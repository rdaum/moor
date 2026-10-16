"""Check cost and credential boundaries before allowing the live scenarios."""

import json
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

from llm_provider import MAX_BODY, MAX_TOKENS, MODEL, Provider, proxy


def request_body():
    return {"model": MODEL, "messages": [{"role": "user", "content": "fixture"}], "stream": False}


class ProviderTests(unittest.TestCase):
    def test_attempt_cap_includes_failures(self):
        provider = Provider(max_requests=1)
        with patch.object(provider, "mock", side_effect=RuntimeError("sensitive fixture text")):
            with self.assertRaisesRegex(ValueError, "^Provider request failed$"):
                provider.request(request_body())
        with self.assertRaisesRegex(ValueError, "request limit"):
            provider.request(request_body())
        self.assertEqual(len(provider.rows), 1)
        self.assertNotIn("sensitive", json.dumps(provider.rows))

    def test_deadline_prevents_requests(self):
        provider = Provider()
        provider.deadline = 0
        with self.assertRaisesRegex(ValueError, "time limit"):
            provider.request(request_body())
        self.assertEqual(provider.rows, [])

    def test_limits_cannot_be_overridden_in_payload(self):
        provider = Provider()
        for extra in ({"model": "another/model"}, {"stream": True},
                      {"max_tokens": MAX_TOKENS + 1}, {"max_tokens": True},
                      {"max_tokens": 0}, {"provider": {"order": ["other"]}},
                      {"tool_choice": "required"}, {"tool_choice": {"type": "function"}}):
            with self.subTest(extra=extra), self.assertRaises(ValueError):
                provider.request(dict(request_body(), **extra))
        self.assertEqual(provider.rows, [])
        self.assertEqual(provider.prepare(request_body())["max_tokens"], MAX_TOKENS)

    def test_proxy_auth_and_size_reject_before_provider(self):
        provider = Provider()
        with proxy(provider) as endpoint:
            for token, body in (("wrong-token", json.dumps(request_body()).encode()),
                                (provider.token, b" " * (MAX_BODY + 1))):
                request = urllib.request.Request(endpoint, data=body, headers={
                    "Authorization": "Bearer " + token})
                with self.assertRaises(urllib.error.HTTPError) as error:
                    urllib.request.urlopen(request)
                self.assertEqual(error.exception.code, 502)
                error.exception.close()
        self.assertEqual(provider.rows, [])

    def test_fixture_never_reads_environment_key(self):
        with patch.dict("os.environ", {"OPENROUTER_API_KEY": "sensitive-fixture"}):
            provider = Provider()
        self.assertIsNone(provider.key)
        self.assertEqual(provider.request(request_body())["choices"][0]["message"]["content"], "READY")


if __name__ == "__main__":
    unittest.main()
