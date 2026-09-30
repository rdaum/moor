#!/usr/bin/env bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# Affero General Public License as published by the Free Software Foundation,
# version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
# details.
#
# You should have received a copy of the GNU Affero General Public License along
# with this program. If not, see <https://www.gnu.org/licenses/>.

# Explicit Debian/Ubuntu provisioning for PostgreSQL-enabled builds and images.
set -euo pipefail
mode=${1:-build}
arch=${2:-$(dpkg --print-architecture)}
case "$mode" in build|runtime) ;; *) echo 'Expected build or runtime' >&2; exit 2;; esac
# shellcheck source=/dev/null
. /etc/os-release
apt-get update
apt-get install -y --no-install-recommends ca-certificates curl
install -d /usr/share/postgresql-common/pgdg
curl --fail --silent --show-error --location \
    https://www.postgresql.org/media/keys/ACCC4CF8.asc \
    -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc
cat >/etc/apt/sources.list.d/moor-pgdg.sources <<SOURCE
Types: deb
URIs: https://apt.postgresql.org/pub/repos/apt
Suites: ${VERSION_CODENAME}-pgdg
Components: main 17
Architectures: $arch
Signed-By: /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc
SOURCE
apt-get update
packages=("libpq5:$arch=17.*")
if [[ "$mode" == build ]]; then packages+=("libpq-dev:$arch=17.*"); fi
apt-get install -y --no-install-recommends "${packages[@]}"
version=$(dpkg-query -W -f='${Version}' "libpq5:$arch")
[[ "$version" == 17.* ]]
