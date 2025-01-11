// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

"use strict";

const assert = require("assert");

const Host = require("..");

const TEST_SIGNING_KEY = `-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEILrkKmddHFUDZqRCnbQsPoW/Wsp0fLqhnv5KNYbcQXtk
-----END PRIVATE KEY-----
`;

const TEST_VERIFYING_KEY = `-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAZQUxGvw8u9CcUHUGLttWFZJaoroXAmQgUGINgbBlVYw=
-----END PUBLIC KEY-----
`;

describe("Moor", () => {
  it("should instantiate ", async () => {
    let host = new Host({
      public_key: TEST_VERIFYING_KEY,
      private_key: TEST_SIGNING_KEY,
    });
    assert.ok(host);
  });

  // TODO: Add more tests here, but they'll rely on a running Moor daemon, so they're not included in the initial version.
  //   Somehow the test runner needs to start the daemon, run the tests, and then stop the daemon.
});
